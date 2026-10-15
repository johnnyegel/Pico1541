use super::monitor;
use super::{join_path, parent_dir, PageState, MenuContext};
use crate::driver::{
    DisplayDriver, FileInfo, FileStorageDriver, IndicatorDriver, RgbLedDriver,
};
use crate::tasks::menu::{Menu, MenuEvent};
use crate::tasks::ui::virtual_directory::VirtualDirectoryManager;
use crate::{InternalCommand, MountPayload};
use core::str::FromStr;
use heapless::{String, Vec};
use pico1541_common::DriveLocation;

pub async fn run_sd_card_menu<D, L, Ind, S, Internal>(
    ctx: &mut MenuContext<'_, D, L, Ind, S, Internal>,
) -> PageState
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
    S: FileStorageDriver + Clone,
    Internal: FileStorageDriver,
{
    run_file_browser_menu(ctx, "SD Card", false).await
}

pub async fn run_internal_menu<D, L, Ind, S, Internal>(
    ctx: &mut MenuContext<'_, D, L, Ind, S, Internal>,
) -> PageState
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
    S: FileStorageDriver + Clone,
    Internal: FileStorageDriver,
{
    run_file_browser_menu(ctx, "Internal", true).await
}

pub fn is_mounted(item_path: &str, mounted_path: Option<&String<128>>) -> bool {
    mounted_path.map_or(false, |m| m.as_str().eq_ignore_ascii_case(item_path))
}

fn format_menu_block(
    infos: &[FileInfo],
    current_path: &str,
    mounted_path: Option<&String<128>>,
) -> Vec<String<64>, 16> {
    let mut block_strings = Vec::<String<64>, 16>::new();
    for info in infos.iter() {
        let mut s = String::<64>::new();
        let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
        let name_trimmed = name_str.trim_matches('\0');
        let file_path = join_path(current_path, name_trimmed);
        if is_mounted(file_path.as_str(), mounted_path) {
            let _ = s.push('*');
        }
        let _ = s.push_str(name_trimmed);
        if info.is_dir {
            let _ = s.push_str("/");
        }
        let _ = block_strings.push(s);
    }
    block_strings
}

async fn run_file_browser_menu<D, L, Ind, S, Internal>(
    ctx: &mut MenuContext<'_, D, L, Ind, S, Internal>,
    title: &str,
    use_internal: bool,
) -> PageState
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
    S: FileStorageDriver + Clone,
    Internal: FileStorageDriver,
{
    let mut cursor: usize = 0;
    let mut scroll_offset: usize = 0;
    let mut cursor_stack = Vec::<(usize, usize), 32>::new();

    loop {
        let current_path_ptr = if use_internal {
            &mut *ctx.internal_path
        } else if title == "SD Card" {
            &mut *ctx.sd_path
        } else {
            &mut *ctx.usb_path
        };

        {
            let loc = ctx.location.lock().await.clone();
            match loc {
                DriveLocation::Root => return PageState::MainMenu,
                DriveLocation::VirtualDirectory { path, use_internal: loc_internal } => {
                    if loc_internal != use_internal {
                        return PageState::MainMenu;
                    }
                    *current_path_ptr = path;
                }
                DriveLocation::DiskImage { path, use_internal: loc_internal } => {
                    if loc_internal != use_internal {
                        return PageState::MainMenu;
                    }
                    let parent = parent_dir(path.as_str());
                    *current_path_ptr = String::from_str(parent).unwrap_or_default();
                    *ctx.mounted_file_path = Some(path);
                }
            }
        }

        let dir_name = {
            let path_str = current_path_ptr.as_str();
            let mut buf = String::<32>::new();
            if path_str == "/" {
                if use_internal {
                    let _ = buf.push_str("INTERNAL");
                } else if let Some(label) = ctx.storage.volume_label() {
                    let _ = buf.push_str(label.as_str());
                } else {
                    let _ = buf.push_str("SD CARD");
                }
            } else if let Some(idx) = path_str.rfind('/') {
                let _ = buf.push_str(&path_str[idx + 1..]);
            } else {
                let _ = buf.push_str(path_str);
            }
            buf
        };

        let mut menu_title = String::<64>::new();
        let path_is_root = current_path_ptr.as_str() == "/";
        if path_is_root && !use_internal {
            if let Some(label) = ctx.storage.volume_label() {
                let _ = core::fmt::write(&mut menu_title, format_args!("SD: {}", label.as_str()));
            } else {
                let _ = menu_title.push_str("SD CARD");
            }
        } else {
            let _ = menu_title.push_str(dir_name.as_str());
        }

        let mut manager = VirtualDirectoryManager {
            use_internal,
            storage: ctx.storage,
            internal: ctx.internal,
            current_path: current_path_ptr,
            context: &ctx.context,
        };

        let mut menu = Menu::<String<64>, 48, _>::new_paged(menu_title.clone());
        menu.set_scroll_offset(scroll_offset);

        // Preload the first few blocks based on cursor
        let current_block = cursor / 16;
        let initial_start_block = current_block.saturating_sub(1);

        let mut files_found = 0;
        for block_idx in initial_start_block..initial_start_block + 4 {
            if let Ok(infos) = manager.ensure_block_loaded(block_idx).await {
                if infos.is_empty() {
                    break;
                }
                files_found += infos.len();
                let mut block_strings = Vec::<String<64>, 16>::new();
                for info in infos.iter() {
                    let mut s = String::<64>::new();
                    let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                    let name_trimmed = name_str.trim_matches('\0');
                    let file_path = join_path(manager.current_path.as_str(), name_trimmed);
                    if is_mounted(file_path.as_str(), ctx.mounted_file_path.as_ref()) {
                        let _ = s.push('*');
                    }
                    let _ = s.push_str(name_trimmed);
                    if info.is_dir {
                        let _ = s.push_str("/");
                    }
                    let _ = block_strings.push(s);
                }
                menu.set_block(block_idx, &block_strings);
            }
        }

        if files_found == 0 && cursor == 0 {
            let mut empty_items = Vec::<&'static str, 1>::new();
            let _ = empty_items.push("No files found");
            let mut empty_menu = Menu::new(menu_title, empty_items);
            loop {
                match empty_menu.run(ctx.display, &ctx.context.action_rx).await {
                    MenuEvent::Timeout => {
                        if !use_internal && !manager.storage.is_available().await {
                            if ctx.context.cmd_tx
                                .try_send(InternalCommand::Unmount)
                                .is_err()
                            {
                                #[cfg(feature = "defmt")]
                                defmt::error!("cmd_tx is FULL for Unmount!");
                            }
                            return PageState::MainMenu;
                        }
                        continue;
                    }
                    MenuEvent::Back => {
                        if manager.current_path.as_str() == "/"
                            || (use_internal && manager.current_path.as_str() == "/files")
                        {
                            if ctx.context.cmd_tx
                                .try_send(InternalCommand::Unmount)
                                .is_err()
                            {
                                #[cfg(feature = "defmt")]
                                defmt::error!("cmd_tx is FULL for Unmount!");
                            }
                            return PageState::MainMenu;
                        } else {
                            let parent = parent_dir(manager.current_path.as_str());
                            let parent_path = String::from_str(parent).unwrap_or_default();
                            *manager.current_path = parent_path.clone();
                            let state = cursor_stack.pop().unwrap_or((0, 0));
                            cursor = state.0;
                            scroll_offset = state.1;
                            *ctx.mounted_file_path = None;
                            let _ = ctx.context.cmd_tx.send(InternalCommand::Mount(
                                MountPayload::VirtualDirectory(parent_path, use_internal),
                            )).await;
                            break;
                        }
                    }
                    MenuEvent::Home => {
                        if ctx.context.cmd_tx
                            .try_send(InternalCommand::Unmount)
                            .is_err()
                        {
                            #[cfg(feature = "defmt")]
                            defmt::error!("cmd_tx is FULL for Unmount!");
                        }
                        return PageState::MainMenu;
                    }
                    MenuEvent::LocationChanged => {
                        let loc = ctx.location.lock().await.clone();
                        match loc {
                            DriveLocation::Root => {
                                return PageState::MainMenu;
                            }
                            DriveLocation::VirtualDirectory {
                                ref path,
                                use_internal: loc_internal,
                            } => {
                                if loc_internal != use_internal {
                                    return PageState::MainMenu;
                                }
                                if !path.as_str().eq_ignore_ascii_case(manager.current_path.as_str()) {
                                    *manager.current_path = path.clone();
                                    cursor_stack.clear();
                                }
                                cursor = 0;
                                scroll_offset = 0;
                                *ctx.mounted_file_path = None;
                                break;
                            }
                            DriveLocation::DiskImage {
                                ref path,
                                use_internal: loc_internal,
                            } => {
                                if loc_internal != use_internal {
                                    return PageState::MainMenu;
                                }
                                let parent = parent_dir(path.as_str());
                                *manager.current_path = String::from_str(parent).unwrap_or_default();
                                *ctx.mounted_file_path = Some(path.clone());
                                cursor = 0;
                                scroll_offset = 0;
                                cursor_stack.clear();
                                break;
                            }
                        }
                    }
                    MenuEvent::Monitor => {
                        if monitor::run_monitor_page(
                            ctx.display,
                            manager.internal,
                            ctx.config,
                            ctx.sys_clock_freq,
                            &ctx.context,
                        )
                        .await
                        {
                            if ctx.context.cmd_tx
                                .try_send(InternalCommand::Unmount)
                                .is_err()
                            {
                                #[cfg(feature = "defmt")]
                                defmt::error!("cmd_tx is FULL for Unmount!");
                            }
                            return PageState::MainMenu;
                        }
                        ctx.display.request_redraw();
                        continue;
                    }
                    _ => {}
                }
            }
            continue;
        }

        // Ensure exact cursor block is loaded
        while let Some(tb) = menu.set_cursor(cursor) {
            if let Ok(infos) = manager.ensure_block_loaded(tb).await {
                if infos.is_empty() {
                    break;
                }
                let block_strings = format_menu_block(
                    &infos,
                    manager.current_path.as_str(),
                    ctx.mounted_file_path.as_ref(),
                );
                menu.set_block(tb, &block_strings);
            } else {
                break;
            }
        }

        let event = loop {
            match menu.run(ctx.display, &ctx.context.action_rx).await {
                MenuEvent::LoadBlock(tb) => {
                    if let Ok(infos) = manager.ensure_block_loaded(tb).await {
                        let block_strings = format_menu_block(
                            &infos,
                            manager.current_path.as_str(),
                            ctx.mounted_file_path.as_ref(),
                        );
                        menu.set_block(tb, &block_strings);
                    }
                }
                MenuEvent::Timeout => {
                    if !use_internal && !manager.storage.is_available().await {
                        break MenuEvent::Timeout;
                    }
                    if let Some(tb) = menu.prefetch_target() {
                        if let Ok(infos) = manager.ensure_block_loaded(tb).await {
                            let block_strings = format_menu_block(
                                &infos,
                                manager.current_path.as_str(),
                                ctx.mounted_file_path.as_ref(),
                            );
                            menu.set_block(tb, &block_strings);
                        }
                    }
                }
                other => break other,
            }
        };

        cursor = menu.selected_index();
        scroll_offset = menu.scroll_offset();

        match event {
            MenuEvent::LoadBlock(_) => {}
            MenuEvent::Timeout => {
                if ctx.context.cmd_tx
                    .try_send(InternalCommand::Unmount)
                    .is_err()
                {
                    #[cfg(feature = "defmt")]
                    defmt::error!("cmd_tx is FULL for Unmount!");
                }

                if !use_internal && title == "SD Card" {
                    *manager.current_path = String::from_str("/").unwrap_or_default();
                }
                return PageState::MainMenu;
            }
            MenuEvent::Selected(idx) => {
                let selected = match manager.fetch_entry_at(idx).await {
                    Some(info) => info,
                    None => {
                        log::error!("Failed to fetch selected item at index {}", idx);
                        #[cfg(feature = "defmt")]
                        defmt::error!("Failed to fetch selected item at index {}", idx);
                        cursor = 0;
                        continue;
                    }
                };

                let selected_name = core::str::from_utf8(&selected.name)
                    .unwrap_or("???")
                    .trim_matches('\0');
                #[cfg(feature = "defmt")]
                defmt::info!("Selected file: \"{}\"", selected_name);
                if selected.is_dir {
                    *ctx.mounted_file_path = None;
                    let dirname = selected_name;
                    let new_path = join_path(manager.current_path.as_str(), dirname);
                    *manager.current_path = new_path.clone();
                    let _ = cursor_stack.push((cursor, menu.scroll_offset()));
                    cursor = 0;
                    scroll_offset = 0;
                    let _ = ctx.context.cmd_tx.send(InternalCommand::Mount(
                        MountPayload::VirtualDirectory(new_path, use_internal),
                    )).await;
                } else {
                    let filename = selected_name;
                    let mut full_path = String::<128>::new();
                    let current_path_val = manager.current_path.as_str();

                    if current_path_val == "/" {
                        let _ = full_path.push_str("/");
                        let _ = full_path.push_str(filename);
                    } else {
                        let _ = full_path.push_str(current_path_val);
                        let _ = full_path.push_str("/");
                        let _ = full_path.push_str(filename);
                    }

                    if is_mounted(full_path.as_str(), ctx.mounted_file_path.as_ref()) {
                        let _ = ctx.context.cmd_tx
                            .send(InternalCommand::Mount(MountPayload::VirtualDirectory(
                                manager.current_path.clone(),
                                use_internal,
                            )))
                            .await;
                        *ctx.mounted_file_path = None;
                        continue;
                    }

                    #[cfg(feature = "defmt")]
                    defmt::info!(
                        "Mounting {} (was {})",
                        full_path.as_str(),
                        ctx.mounted_file_path
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("None")
                    );

                    log::info!("Selected file: {:?}", selected);
                    ctx.display.clear().await;
                    ctx.display.print(1, "   Mounting...  ").await;
                    ctx.display.request_redraw();

                    let is_d64 = filename.ends_with(".d64") || filename.ends_with(".D64");
                    let is_g64 = filename.ends_with(".g64") || filename.ends_with(".G64");
                    let is_nib = filename.ends_with(".nib") || filename.ends_with(".NIB");

                    if is_d64 {
                        #[cfg(feature = "defmt")]
                        defmt::info!("Sending Mount for D64...");
                        let _ = ctx.context.cmd_tx
                            .send(InternalCommand::Mount(MountPayload::D64(
                                full_path.clone(),
                                manager.use_internal,
                            )))
                            .await;
                        *ctx.mounted_file_path = Some(full_path);
                    } else if is_g64 {
                        #[cfg(feature = "defmt")]
                        defmt::info!("Sending Mount for G64...");
                        let _ = ctx.context.cmd_tx
                            .send(InternalCommand::Mount(MountPayload::G64(
                                full_path.clone(),
                                manager.use_internal,
                            )))
                            .await;
                        *ctx.mounted_file_path = Some(full_path);
                    } else if is_nib {
                        #[cfg(feature = "defmt")]
                        defmt::info!("Sending Mount for NIB...");
                        let _ = ctx.context.cmd_tx
                            .send(InternalCommand::Mount(MountPayload::Nib(
                                full_path.clone(),
                                manager.use_internal,
                            )))
                            .await;
                        *ctx.mounted_file_path = Some(full_path);
                    } else {
                        // Non-image file (PRG, SEQ, …) — mount as single-file virtual disk.
                        #[cfg(feature = "defmt")]
                        defmt::info!("Sending Mount for PRG/single-file...");
                        let _ = ctx
                            .context
                            .cmd_tx
                            .send(InternalCommand::Mount(MountPayload::Prg(
                                full_path.clone(),
                                manager.use_internal,
                            )))
                            .await;
                        *ctx.mounted_file_path = Some(full_path);
                    }
                }
            }
            MenuEvent::Back => {
                if manager.current_path.as_str() == "/"
                    || (use_internal && manager.current_path.as_str() == "/files")
                {
                    if ctx.context.cmd_tx
                        .try_send(InternalCommand::Unmount)
                        .is_err()
                    {
                        #[cfg(feature = "defmt")]
                        defmt::error!("cmd_tx is FULL for Unmount!");
                    }

                    return PageState::MainMenu;
                }

                let parent = parent_dir(manager.current_path.as_str());
                let parent_path = String::from_str(parent).unwrap_or_default();
                *manager.current_path = parent_path.clone();

                let state = cursor_stack.pop().unwrap_or((0, 0));
                cursor = state.0;
                scroll_offset = state.1;
                *ctx.mounted_file_path = None;
                let _ = ctx.context.cmd_tx.send(InternalCommand::Mount(
                    MountPayload::VirtualDirectory(parent_path, use_internal),
                )).await;
            }
            MenuEvent::Home => {
                if ctx.context.cmd_tx
                    .try_send(InternalCommand::Unmount)
                    .is_err()
                {
                    #[cfg(feature = "defmt")]
                    defmt::error!("cmd_tx is FULL for Unmount!");
                }

                return PageState::MainMenu;
            }
            MenuEvent::LocationChanged => {
                let loc = ctx.location.lock().await.clone();
                match loc {
                    DriveLocation::Root => {
                        return PageState::MainMenu;
                    }
                    DriveLocation::VirtualDirectory {
                        ref path,
                        use_internal: loc_internal,
                    } => {
                        if loc_internal != use_internal {
                            return PageState::MainMenu;
                        }
                        if !path.as_str().eq_ignore_ascii_case(manager.current_path.as_str()) {
                            *manager.current_path = path.clone();
                            cursor_stack.clear();
                        }
                        cursor = 0;
                        scroll_offset = 0;
                        *ctx.mounted_file_path = None;
                        continue;
                    }
                    DriveLocation::DiskImage {
                        ref path,
                        use_internal: loc_internal,
                    } => {
                        if loc_internal != use_internal {
                            return PageState::MainMenu;
                        }
                        let parent = parent_dir(path.as_str());
                        if !parent.eq_ignore_ascii_case(manager.current_path.as_str()) {
                            *manager.current_path = String::from_str(parent).unwrap_or_default();
                            cursor = 0;
                            scroll_offset = 0;
                            cursor_stack.clear();
                        }
                        *ctx.mounted_file_path = Some(path.clone());
                        continue;
                    }
                }
            }
            MenuEvent::Monitor => {
                if monitor::run_monitor_page(
                    ctx.display,
                    manager.internal,
                    ctx.config,
                    ctx.sys_clock_freq,
                    &ctx.context,
                )
                .await
                {
                    if ctx.context.cmd_tx
                        .try_send(InternalCommand::Unmount)
                        .is_err()
                    {
                        #[cfg(feature = "defmt")]
                        defmt::error!("cmd_tx is FULL for Unmount!");
                    }

                    return PageState::MainMenu;
                }
                ctx.display.request_redraw();
            }
            _ => {}
        }
    }
}
