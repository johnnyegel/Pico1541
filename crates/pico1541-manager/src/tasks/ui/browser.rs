use heapless::{String, Vec};
use core::str::FromStr;
use embassy_time::{Duration, Timer};

use crate::driver::{DisplayDriver, FileStorageDriver, FileInfo, RgbLedDriver, StatusLedMode};
use crate::tasks::menu::{Menu, MenuEvent};
use super::{PageState, parent_dir, join_path, UiContext};
use crate::storage::PagedDirectorySourceTrait;
use super::monitor;

pub async fn run_sd_card_menu<D, L, Ind, S, Internal, File>(ctx: &mut UiContext<'_, D, L, Ind, S, Internal, File>) -> PageState 
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    run_file_browser_menu(ctx, "SD Card", false).await
}

pub async fn run_internal_menu<D, L, Ind, S, Internal, File>(ctx: &mut UiContext<'_, D, L, Ind, S, Internal, File>) -> PageState
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    run_file_browser_menu(ctx, "Internal", true).await
}

async fn run_file_browser_menu<D, L, Ind, S, Internal, File>(ctx: &mut UiContext<'_, D, L, Ind, S, Internal, File>, title: &str, use_internal: bool) -> PageState
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{

        let mut cursor: usize = 0;
        let mut scroll_offset: usize = 0;
        let mut cursor_stack = heapless::Vec::<(usize, usize), 32>::new();
        loop {
            let current_path = if use_internal {
                ctx.internal_path.clone()
            } else if title == "SD Card" {
                ctx.sd_path.clone()
            } else {
                ctx.usb_path.clone()
            };

            let path_str = current_path.as_str();

            let current_block = cursor / 16;
            let initial_start_block = current_block.saturating_sub(1);
            let initial_start_idx = initial_start_block * 16;

            // Load the entries around the cursor position (up to 64 entries = 4 blocks)
            let mut initial_infos = Vec::<FileInfo, 64>::new();
            let load_res = if use_internal {
                ctx.internal.list_dir_range(current_path.as_str(), initial_start_idx, 64, |info| {
                    let _ = initial_infos.push(info);
                }).await
            } else {
                ctx.storage.list_dir_range(current_path.as_str(), initial_start_idx, 64, |info| {
                    let _ = initial_infos.push(info);
                }).await
            };

            if let Err(e) = load_res {
                log::error!("Error listing dir: {:?}", e);
                let current_path_mut = if use_internal {
                    &mut *ctx.internal_path
                } else if title == "SD Card" {
                    &mut *ctx.sd_path
                } else {
                    &mut *ctx.usb_path
                };
                *current_path_mut = String::from_str("/").unwrap();

                ctx.display.clear().await;
                ctx.display.print(1, "  Read Error!").await;
                ctx.display.request_redraw();
                Timer::after(Duration::from_secs(2)).await;
                if !use_internal {
                    let mut disk = ctx._disk_image.lock().await;
                    *disk = None;
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    ctx.leds.set_status(StatusLedMode::Unmounted);
                }
                return PageState::MainMenu;
            }

            let mut dir_name_buf = String::<32>::new();
            let dir_name = if path_str == "/" {
                if use_internal {
                    "INTERNAL"
                } else if let Some(label) = ctx.storage.volume_label() {
                    let _ = dir_name_buf.push_str(label.as_str());
                    dir_name_buf.as_str()
                } else {
                    "SD CARD"
                }
            } else if let Some(idx) = path_str.rfind('/') {
                &path_str[idx + 1..]
            } else {
                path_str
            };

            // Build directory entries for the virtual disk using PagedDirectorySource.
            // Distribute the fetched entries into blocks; set_block auto-detects end-of-dir.
            let mut dir_source = crate::storage::PagedDirectorySource::new(dir_name.as_bytes());
            for (block_idx, chunk) in initial_infos.chunks(16).enumerate() {
                let mut block_entries = Vec::<crate::storage::DirectoryEntry, 16>::new();
                for info in chunk {
                    let mut name_len = 0;
                    while name_len < info.name.len() && info.name[name_len] != 0 {
                        name_len += 1;
                    }
                    let entry = crate::storage::DirectoryEntry::from_file_info(&info.name[..name_len], info.is_dir, info.size);
                    let _ = block_entries.push(entry);
                }
                dir_source.set_block(initial_start_block + block_idx, &block_entries);
            }

            let vdisk = crate::formats::VirtualDiskImage::new(dir_source, dir_name.as_bytes());

            if ctx.mounted_file_path.is_none() {
                {
                    let mut mounted_dir_guard = ctx.mounted_dir.lock().await;
                    mounted_dir_guard.0 = current_path.clone();
                    mounted_dir_guard.1 = use_internal;
                }
                {
                    let mut disk_lock = ctx._disk_image.lock().await;
                    *disk_lock = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
                    let mut default_name: heapless::String<128> = heapless::String::new();
                    let _ = default_name.push_str("Internal Virtual Disk");
                    crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(default_name)).await;
                }
                ctx.leds.set_status(StatusLedMode::VirtualDisk);
            }

            {
                let mut viewed_dir_guard = ctx.viewed_dir.lock().await;
                viewed_dir_guard.0 = current_path.clone();
                viewed_dir_guard.1 = use_internal;
            }

            let mut menu_title = String::<64>::new();
            if path_str == "/" && !use_internal {
                if let Some(label) = ctx.storage.volume_label() {
                    let _ = core::fmt::write(&mut menu_title, format_args!("SD: {}", label.as_str()));
                } else {
                    let _ = menu_title.push_str("SD CARD");
                }
            } else {
                let _ = menu_title.push_str(dir_name);
            }
            let mut menu = Menu::<String<64>, 64, _>::new_paged(menu_title.clone());
            menu.set_scroll_offset(scroll_offset);

            for (block_idx, chunk) in initial_infos.chunks(16).enumerate() {
                let mut block_strings = Vec::<String<64>, 16>::new();
                for info in chunk {
                    let mut s = String::<64>::new();
                    let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                    let name_trimmed = name_str.trim_matches('\0');
                    let file_path = join_path(current_path.as_str(), name_trimmed);
                    if Some(file_path) == *ctx.mounted_file_path {
                        let _ = s.push('*');
                    }
                    let _ = s.push_str(name_trimmed);
                    if info.is_dir {
                        let _ = s.push_str("/");
                    }
                    
                    let _ = block_strings.push(s);
                }
                menu.set_block(initial_start_block + block_idx, &block_strings);
            }

            if initial_infos.is_empty() {
                let mut empty_items = Vec::<&'static str, 1>::new();
                let _ = empty_items.push("No files found");
                let mut menu = Menu::new(menu_title, empty_items);
                match menu.run(ctx.display).await {
                    MenuEvent::Back => {
                        let current_path_mut = if use_internal {
                            &mut *ctx.internal_path
                        } else if title == "SD Card" {
                            &mut *ctx.sd_path
                        } else {
                            &mut *ctx.usb_path
                        };
                        if current_path_mut.as_str() == "/" {
                            let mut disk = ctx._disk_image.lock().await;
                            *disk = None;
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                            ctx.leds.set_status(StatusLedMode::Unmounted);
                            return PageState::MainMenu;
                        } else {
                            let parent = parent_dir(current_path_mut.as_str());
                            *current_path_mut = String::from_str(parent).unwrap();
                            let state = cursor_stack.pop().unwrap_or((0, 0));
                            cursor = state.0;
                            scroll_offset = state.1;
                            *ctx.mounted_file_path = None;
                            continue;
                        }
                    }
                    MenuEvent::Home => {
                        let mut disk = ctx._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                        ctx.leds.set_status(StatusLedMode::Unmounted);
                        return PageState::MainMenu;
                    }
                    MenuEvent::Monitor => {
                        if monitor::run_monitor_page(ctx.display, ctx.internal, ctx.config).await {
                            let mut disk = ctx._disk_image.lock().await;
                            *disk = None;
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                            ctx.leds.set_status(StatusLedMode::Unmounted);
                            return PageState::MainMenu;
                        }
                        ctx.display.request_redraw();
                        continue;
                    }
                    _ => {}
                }
                if !use_internal {
                    let mut disk = ctx._disk_image.lock().await;
                    *disk = None;
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    ctx.leds.set_status(StatusLedMode::Unmounted);
                }
                return PageState::MainMenu;
            }

            // Build paged menu. total_count is discovered lazily as blocks are fetched.
            let mut menu = Menu::<String<64>, 48, _>::new_paged(menu_title);
            menu.set_scroll_offset(scroll_offset);

            // Load initial blocks into the menu display buffer.
            for (block_idx, chunk) in initial_infos.chunks(16).enumerate() {
                let mut block_strings = Vec::<String<64>, 16>::new();
                for info in chunk {
                    let mut s = String::<64>::new();
                    let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                    let name_trimmed = name_str.trim_matches('\0');
                    let file_path = join_path(current_path.as_str(), name_trimmed);
                    if Some(file_path) == *ctx.mounted_file_path {
                        let _ = s.push('*');
                    }
                    let _ = s.push_str(name_trimmed);
                    if info.is_dir {
                        let _ = s.push_str("/");
                    }
                    
                    let _ = block_strings.push(s);
                }
                menu.set_block(initial_start_block + block_idx, &block_strings);
            }

            // Ensure cursor block is loaded
            while let Some(tb) = menu.set_cursor(cursor) {
                    let mut block_infos = Vec::<FileInfo, 16>::new();
                    let fetch_res = if use_internal {
                        ctx.internal.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                            let _ = block_infos.push(info);
                        }).await
                    } else {
                        ctx.storage.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                            let _ = block_infos.push(info);
                        }).await
                    };
                    
                    if fetch_res.is_ok() {
                        let mut block_strings = Vec::<String<64>, 16>::new();
                        for info in block_infos.iter() {
                            let mut s = String::<64>::new();
                            let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                            let name_trimmed = name_str.trim_matches('\0');
                            let file_path = join_path(current_path.as_str(), name_trimmed);
                            if Some(file_path) == *ctx.mounted_file_path {
                                let _ = s.push('*');
                            }
                            let _ = s.push_str(name_trimmed);
                            if info.is_dir {
                                let _ = s.push_str("/");
                            }
                            let _ = block_strings.push(s);
                        }
                        menu.set_block(tb, &block_strings);
                    } else {
                        break;
                    }
                }

            let event = loop {
                match menu.run(ctx.display).await {
                    MenuEvent::LoadBlock(tb) => {
                        let mut block_infos = Vec::<FileInfo, 16>::new();
                        let fetch_res = if use_internal {
                            ctx.internal.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                let _ = block_infos.push(info);
                            }).await
                        } else {
                            ctx.storage.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                let _ = block_infos.push(info);
                            }).await
                        };
                        
                        if fetch_res.is_ok() {
                            let mut block_strings = Vec::<String<64>, 16>::new();
                            for info in block_infos.iter() {
                                let mut s = String::<64>::new();
                                let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                                let name_trimmed = name_str.trim_matches('\0');
                                let file_path = join_path(current_path.as_str(), name_trimmed);
                                if Some(file_path) == *ctx.mounted_file_path {
                                    let _ = s.push('*');
                                }
                                let _ = s.push_str(name_trimmed);
                                if info.is_dir {
                                    let _ = s.push_str("/");
                                }
                                
                                let _ = block_strings.push(s);
                            }
                            menu.set_block(tb, &block_strings);
                        }
                    }
                    MenuEvent::Timeout => {
                        if !use_internal && !ctx.storage.is_available().await {
                            break MenuEvent::Timeout;
                        }
                        
                        if let Some(tb) = menu.prefetch_target() {
                            let mut block_infos = Vec::<FileInfo, 16>::new();
                            let fetch_res = if use_internal {
                                ctx.internal.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                    let _ = block_infos.push(info);
                                }).await
                            } else {
                                ctx.storage.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                    let _ = block_infos.push(info);
                                }).await
                            };
                            
                            if fetch_res.is_ok() {
                                let mut block_strings = Vec::<String<64>, 16>::new();
                                for info in block_infos.iter() {
                                    let mut s = String::<64>::new();
                                    let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                                    let name_trimmed = name_str.trim_matches('\0');
                                    let file_path = join_path(current_path.as_str(), name_trimmed);
                                    if Some(file_path) == *ctx.mounted_file_path {
                                        let _ = s.push('*');
                                    }
                                    let _ = s.push_str(name_trimmed);
                                    if info.is_dir {
                                        let _ = s.push_str("/");
                                    }
                                    let _ = block_strings.push(s);
                                }
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
                    let mut disk = ctx._disk_image.lock().await;
                    *disk = None;
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    ctx.leds.set_status(StatusLedMode::Unmounted);
                    if !use_internal && title == "SD Card" {
                        *ctx.sd_path = String::from_str("/").unwrap();
                    }
                    return PageState::MainMenu;
                }
                MenuEvent::Selected(idx) => {
                    let mut selected_infos = Vec::<FileInfo, 1>::new();
                    let fetch_res = if use_internal {
                        ctx.internal.list_dir_range(current_path.as_str(), idx, 1, |info| {
                            let _ = selected_infos.push(info);
                        }).await
                    } else {
                        ctx.storage.list_dir_range(current_path.as_str(), idx, 1, |info| {
                            let _ = selected_infos.push(info);
                        }).await
                    };
                    
                    if fetch_res.is_err() || selected_infos.is_empty() {
                        log::error!("Failed to fetch selected item at index {}", idx);
                        cursor = 0;
                        continue;
                    }
                    let selected = selected_infos[0];
                    if selected.is_dir {
                        *ctx.mounted_file_path = None;
                        let dirname = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let current_path_mut = if use_internal {
                            &mut *ctx.internal_path
                        } else if title == "SD Card" {
                            &mut *ctx.sd_path
                        } else {
                            &mut *ctx.usb_path
                        };
                        let new_path = join_path(current_path_mut.as_str(), dirname);
                        *current_path_mut = new_path;
                        let _ = cursor_stack.push((cursor, menu.scroll_offset()));
                        cursor = 0;
                        scroll_offset = 0;
                    } else {
                        let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let mut full_path = String::<128>::new();
                        let current_path_val = if use_internal {
                            ctx.internal_path.as_str()
                        } else if title == "SD Card" {
                            ctx.sd_path.as_str()
                        } else {
                            ctx.usb_path.as_str()
                        };
                        if current_path_val == "/" {
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        } else {
                            let _ = full_path.push_str(current_path_val);
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        }

                        if Some(full_path.clone()) == *ctx.mounted_file_path {
                            let mut disk = ctx._disk_image.lock().await;
                            *disk = None;
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                            ctx.leds.set_status(StatusLedMode::Unmounted);
                            *ctx.mounted_file_path = None;
                            continue;
                        }

                        log::info!("Selected file: {:?}", selected);
                        ctx.display.clear().await;
                        ctx.display.print(1, "   Mounting...  ").await;
                        ctx.display.request_redraw();

                        let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let mut full_path = String::<128>::new();
                        let current_path_val = if use_internal {
                            ctx.internal_path.as_str()
                        } else if title == "SD Card" {
                            ctx.sd_path.as_str()
                        } else {
                            ctx.usb_path.as_str()
                        };
                        if current_path_val == "/" {
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        } else {
                            let _ = full_path.push_str(current_path_val);
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        }

                        let is_d64 = filename.ends_with(".d64") || filename.ends_with(".D64");
                        let is_g64 = filename.ends_with(".g64") || filename.ends_with(".G64");

                        if is_d64 {
                            if use_internal {
                                let file = ctx.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File2(file);
                                let d64 = crate::formats::D64Image::new(runtime_storage);
                                    {
                                        let mut disk = ctx._disk_image.lock().await;
                                        *disk = Some(crate::formats::AnyDiskImage::D64(d64));
                                        crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                    }
                                    ctx.leds.set_status(StatusLedMode::NonAccurateDisk);
                                    *ctx.mounted_file_path = Some(full_path.clone());
                                    continue;
                            } else {
                                let file_handle = ctx.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File(file_handle);
                                let d64 = crate::formats::D64Image::new(runtime_storage);
                                {
                                    let mut disk = ctx._disk_image.lock().await;
                                    *disk = Some(crate::formats::AnyDiskImage::D64(d64));
                                    crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                }
                                ctx.leds.set_status(StatusLedMode::NonAccurateDisk);
                                *ctx.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        } else if is_g64 {
                            if use_internal {
                                let file = ctx.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File2(file);
                                if let Ok(g64) = crate::formats::G64Image::from_storage(runtime_storage).await {
                                    {
                                        let mut disk = ctx._disk_image.lock().await;
                                        *disk = Some(crate::formats::AnyDiskImage::G64(g64));
                                        crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                    }
                                    ctx.leds.set_status(StatusLedMode::AccurateDisk);
                                    *ctx.mounted_file_path = Some(full_path.clone());
                                }
                                continue;
                            } else {
                                let file_handle = ctx.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File(file_handle);
                                if let Ok(g64) = crate::formats::G64Image::from_storage(runtime_storage).await {
                                    {
                                        let mut disk = ctx._disk_image.lock().await;
                                        *disk = Some(crate::formats::AnyDiskImage::G64(g64));
                                        crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                    }
                                    ctx.leds.set_status(StatusLedMode::AccurateDisk);
                                    *ctx.mounted_file_path = Some(full_path.clone());
                                }
                                continue;
                            }
                        } else {
                            let mut name_len = 0;
                            while name_len < selected.name.len() && selected.name[name_len] != 0 {
                                name_len += 1;
                            }
                            let entry = crate::storage::DirectoryEntry::from_file_info(&selected.name[..name_len], false, selected.size);
                            let mut dir_source = crate::storage::PagedDirectorySource::new(filename.as_bytes());
                            let mut block_entries = Vec::<crate::storage::DirectoryEntry, 16>::new();
                            let _ = block_entries.push(entry);
                            dir_source.set_block(0, &block_entries);

                            let mut vdisk = crate::formats::VirtualDiskImage::new(dir_source, "INTERNAL".as_bytes());

                            if use_internal {
                                let file = ctx.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File2(file);
                                vdisk.file_storage = Some(runtime_storage);
                                vdisk.set_shortcut_entry(Some(entry));
                                {
                                    let mut disk = ctx._disk_image.lock().await;
                                    *disk = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
                                    crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                }
                                ctx.leds.set_status(StatusLedMode::VirtualDisk);
                                *ctx.mounted_file_path = Some(full_path.clone());
                                continue;
                            } else {
                                let file_handle = ctx.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = crate::storage::RuntimeStorage::File(file_handle);
                                vdisk.file_storage = Some(runtime_storage);
                                vdisk.set_shortcut_entry(Some(entry));
                                {
                                    let mut disk = ctx._disk_image.lock().await;
                                    *disk = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
                                    crate::CMD_CHANNEL.send(crate::InternalCommand::ImageMounted(full_path.clone())).await;
                                }
                                ctx.leds.set_status(StatusLedMode::VirtualDisk);
                                *ctx.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        }
                    }
                }
                MenuEvent::ContextAction(idx) => {
                    if use_internal { continue; }
                    let mut selected_infos = Vec::<FileInfo, 1>::new();
                    let fetch_res = ctx.storage.list_dir_range(current_path.as_str(), idx, 1, |info| {
                        let _ = selected_infos.push(info);
                    }).await;
                    
                    if fetch_res.is_err() || selected_infos.is_empty() {
                        continue;
                    }
                    let selected = selected_infos[0];
                    if selected.is_dir { continue; }

                    let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                    if filename.ends_with(".bin") || filename.ends_with(".rom") || filename.ends_with(".BIN") || filename.ends_with(".ROM") {
                        if use_internal {
                            let mut options = Vec::<String<32>, 2>::new();
                            options.push(String::from_str("Abort").unwrap()).unwrap();
                            options.push(String::from_str("Delete").unwrap()).unwrap();
                            let mut ctx_menu = Menu::new("Delete ROM?", options);
                            loop {
                                match ctx_menu.run(ctx.display).await {
                                    MenuEvent::Selected(opt_idx) => {
                                        if opt_idx == 1 {
                                            let _ = crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::DeleteRom(heapless::String::<32>::from_str(filename).unwrap()));
                                            ctx.display.clear().await;
                                            ctx.display.print(2, "   ROM Deleted!").await;
                                            ctx.display.request_redraw();
                                            Timer::after(Duration::from_millis(1500)).await;
                                        }
                                        break;
                                    }
                                    MenuEvent::Back | MenuEvent::Home => break,
                                    MenuEvent::Monitor => {
                                        let _ = monitor::run_monitor_page(ctx.display, ctx.internal, ctx.config).await;
                                        ctx.display.request_redraw();
                                        continue;
                                    }
                                    MenuEvent::Timeout => continue,
                                    MenuEvent::ContextAction(_) => continue,
                                    MenuEvent::LoadBlock(_) => {}
                                }
                            }
                        } else if selected.size >= 16384 && selected.size.is_multiple_of(16384) && selected.size <= 65536 {
                            let mut options = Vec::<String<32>, 5>::new();
                            options.push(String::from_str("Abort").unwrap()).unwrap();
                            let num_banks = selected.size / 16384;
                            if num_banks == 1 {
                                options.push(String::from_str("Install").unwrap()).unwrap();
                            } else {
                                for b in 1..=num_banks {
                                    let mut s = String::<32>::new();
                                    core::fmt::write(&mut s, format_args!("Install Bank {}", b)).unwrap();
                                    options.push(s).unwrap();
                                }
                            }
                            let mut ctx_menu = Menu::new("Install ROM?", options);
                            loop {
                                match ctx_menu.run(ctx.display).await {
                                    MenuEvent::Selected(opt_idx) => {
                                        if opt_idx > 0 {
                                            let bank = opt_idx as u32; // 1-indexed bank
                                            let path_str = heapless::String::<128>::from_str(current_path.as_str()).unwrap();
                                            install_rom_from_sd(ctx, path_str.as_str(), filename, bank).await;
                                        }
                                        break;
                                    }
                                    MenuEvent::Back | MenuEvent::Home => break,
                                    MenuEvent::Monitor => {
                                        let _ = monitor::run_monitor_page(ctx.display, ctx.internal, ctx.config).await;
                                        ctx.display.request_redraw();
                                        continue;
                                    }
                                    MenuEvent::Timeout => continue,
                                    MenuEvent::ContextAction(_) => continue, // ignore repeated hold events
                                    MenuEvent::LoadBlock(_) => {} // context menu doesn't need blocks
                                }
                            }
                        }
                    }
                }
                MenuEvent::Back => {
                    let current_path_mut = if use_internal {
                        &mut *ctx.internal_path
                    } else if title == "SD Card" {
                        &mut *ctx.sd_path
                    } else {
                        &mut *ctx.usb_path
                    };
                    let is_root = if use_internal {
                        current_path_mut.as_str() == "/files"
                    } else {
                        current_path_mut.as_str() == "/"
                    };

                    if is_root {
                        let mut disk = ctx._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                        ctx.leds.set_status(StatusLedMode::Unmounted);
                        *ctx.mounted_file_path = None;
                        return PageState::MainMenu;
                    } else {
                        let parent = parent_dir(current_path_mut.as_str());
                        *current_path_mut = String::from_str(parent).unwrap();
                        *ctx.mounted_file_path = None;
                        let state = cursor_stack.pop().unwrap_or((0, 0));
                        cursor = state.0;
                        scroll_offset = state.1;
                        continue;
                    }
                }
                MenuEvent::Home => {
                    let mut disk = ctx._disk_image.lock().await;
                    *disk = None;
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    ctx.leds.set_status(StatusLedMode::Unmounted);
                    *ctx.mounted_file_path = None;
                    return PageState::MainMenu;
                }
                MenuEvent::Monitor => {
                    if monitor::run_monitor_page(ctx.display, ctx.internal, ctx.config).await {
                        let mut disk = ctx._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                        ctx.leds.set_status(StatusLedMode::Unmounted);
                        *ctx.mounted_file_path = None;
                        return PageState::MainMenu;
                    }
                    ctx.display.request_redraw();
                    continue;
                }
            }
        }
    }






async fn install_rom_from_sd<D, L, Ind, S, Internal, File>(ctx: &mut UiContext<'_, D, L, Ind, S, Internal, File>, current_path: &str, filename: &str, bank: u32) 
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{

        use embedded_io_async::SeekFrom;
        ctx.display.clear().await;
        ctx.display.print(1, " Installing ROM...").await;
        ctx.display.request_redraw();

        let full_path = join_path(current_path, filename);
        let mut file = ctx.storage.open_file_handle(full_path.as_str());

        let offset = (bank - 1) * 16384;
        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)).await {
            log::error!("Failed to seek to ROM bank: {:?}", e);
            ctx.display.print(2, " Seek Error!").await;
            ctx.display.request_redraw();
            Timer::after(Duration::from_secs(2)).await;
            return;
        }

        let mut dest_filename = String::<32>::new();
        if filename.ends_with(".bin") || filename.ends_with(".BIN") {
            let base = &filename[..filename.len() - 4];
            let _ = core::fmt::write(&mut dest_filename, format_args!("{}-B{}.bin", base, bank));
        } else if filename.ends_with(".rom") || filename.ends_with(".ROM") {
            let base = &filename[..filename.len() - 4];
            let _ = core::fmt::write(&mut dest_filename, format_args!("{}-B{}.rom", base, bank));
        } else {
            let _ = dest_filename.push_str(filename);
        }

        let mut buf = [0u8; 256];
        let mut bytes_read = 0;

        let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomStart {
            filename: dest_filename.clone(),
            size: 16384,
            offset: 0,
        }).await;

        while bytes_read < 16384 {
            let to_read = (16384 - bytes_read).min(256);
            if let Ok(n) = file.read(&mut buf[..to_read]).await {
                if n == 0 { break; }
                let mut chunk = heapless::Vec::new();
                let _ = chunk.extend_from_slice(&buf[..n]);
                let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomChunk(chunk)).await;
                bytes_read += n;
                
                if bytes_read % 2048 == 0 {
                    let mut prog = String::<32>::new();
                    let _ = core::fmt::write(&mut prog, format_args!(" {} / 16384", bytes_read));
                    ctx.display.print(3, &prog).await;
                    ctx.display.request_redraw();
                }
            } else {
                break;
            }
        }

        let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomFinish).await;
        
        ctx.display.print(4, " Done!").await;
        ctx.display.request_redraw();
        Timer::after(Duration::from_secs(1)).await;
    }

