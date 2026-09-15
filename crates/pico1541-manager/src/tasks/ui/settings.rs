use core::str::FromStr;
use embassy_time::{Duration, Timer};
use heapless::{String, Vec};
use portable_atomic::Ordering;

use super::PageState;
use crate::driver::{DisplayDriver, FileStorageDriver};
use crate::tasks::menu::{Menu, MenuEvent};
use pico1541_common::snooper::SnooperConfig;

pub async fn run_settings_menu<D, Internal>(
    display: &mut D,
    _internal: &mut Internal,
    config: &SnooperConfig,
) -> PageState
where
    D: DisplayDriver,
    Internal: FileStorageDriver,
{
    let mut cursor = 0;
    loop {
        let mut items = Vec::<String<32>, 3>::new();

        let mut id_str = String::<32>::new();
        let _ = core::fmt::write(
            &mut id_str,
            format_args!("Drive ID: {}", config.device_id.load(Ordering::Relaxed)),
        );
        let _ = items.push(id_str);

        let _ = items.push(String::from_str("Select ROM").unwrap());

        let _ = items.push(String::from_str("Monitor").unwrap());

        let mut menu = Menu::new("--- Settings ---", items);
        menu.set_cursor(cursor);

        match menu.run(display).await {
            MenuEvent::Selected(idx) => {
                cursor = idx;
                match idx {
                    0 => {
                        return PageState::DeviceIdSelect;
                    }
                    1 => {
                        return PageState::BootRomSelect;
                    }
                    2 => {
                        return PageState::Monitor;
                    }
                    _ => {}
                }
            }
            MenuEvent::Back => return PageState::MainMenu,
            MenuEvent::Home => return PageState::MainMenu,
            MenuEvent::Monitor => {
                return PageState::Monitor;
            }
            MenuEvent::LoadBlock(_) => {}
            MenuEvent::Timeout => {
                cursor = menu.selected_index();
            }
            _ => {}
        }
    }
}

pub async fn run_device_id_menu<D, Internal>(
    display: &mut D,
    _internal: &mut Internal,
    config: &SnooperConfig,
) -> PageState
where
    D: DisplayDriver,
    Internal: FileStorageDriver,
{
    let mut cursor = (config.device_id.load(Ordering::Relaxed) - 8) as usize;
    loop {
        let current_id = config.device_id.load(Ordering::Relaxed);
        let mut items = Vec::<String<16>, 4>::new();

        for id in 8..=11 {
            let mut s = String::<16>::new();
            if id == current_id {
                let _ = s.push_str("*");
            }
            let _ = core::fmt::write(&mut s, format_args!("{}", id));
            let _ = items.push(s);
        }

        let mut menu = Menu::new("--- Drive ID ---", items);
        menu.set_cursor(cursor);

        match menu.run(display).await {
            MenuEvent::Selected(idx) => {
                let new_id = (idx + 8) as u8;
                config.update_device_id(new_id);
                let _ =
                    crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::SaveDriveId(new_id));
                display.clear().await;
                display.print(2, "   Drive ID Saved!").await;
                display.request_redraw();
                Timer::after(Duration::from_millis(1500)).await;
                return PageState::Settings;
            }
            MenuEvent::Back => return PageState::Settings,
            MenuEvent::Home => return PageState::MainMenu,
            MenuEvent::Monitor => {
                return PageState::Monitor;
            }
            MenuEvent::LoadBlock(_) => {}
            MenuEvent::Timeout => {
                cursor = menu.selected_index();
            }
            MenuEvent::ContextAction(_) => {}
        }
    }
}

pub async fn run_select_boot_rom_menu<D, Internal>(
    display: &mut D,
    internal: &mut Internal,
    _config: &SnooperConfig,
    rom_name: &mut String<32>,
) -> PageState
where
    D: DisplayDriver,
    Internal: FileStorageDriver,
{
    let mut cursor = 0;
    loop {
        let mut items = Vec::<String<32>, 48>::new();
        let mut infos = Vec::<crate::driver::FileInfo, 48>::new();

        let _ = internal
            .list_dir_range("/roms", 0, 48, |info| {
                let _ = infos.push(info);
            })
            .await;

        let mut roms_added = 0;
        for info in &infos {
            if !info.is_dir {
                let name = core::str::from_utf8(&info.name)
                    .unwrap_or("???")
                    .trim_matches('\0');
                if name.ends_with(".bin")
                    || name.ends_with(".rom")
                    || name.ends_with(".BIN")
                    || name.ends_with(".ROM")
                {
                    let mut s = String::<32>::new();
                    if name == rom_name.as_str() {
                        let _ = s.push('*');
                    }
                    let _ = s.push_str(name);
                    let _ = items.push(s);
                    roms_added += 1;
                }
            }
        }

        if roms_added == 0 {
            let _ = items.push(String::from_str("No roms installed").unwrap());
        }

        let mut menu = Menu::new("--- Boot ROM ---", items);
        menu.set_cursor(cursor);

        match menu.run(display).await {
            MenuEvent::Selected(idx) => {
                if roms_added == 0 {
                    return PageState::Settings;
                }
                let name_with_star = menu.get_item(idx).unwrap();
                let name = if let Some(stripped) = name_with_star.strip_prefix('*') {
                    stripped
                } else {
                    name_with_star.as_str()
                };
                *rom_name = String::from_str(name).unwrap();
                let _ = crate::SETTINGS_CHANNEL
                    .try_send(crate::SettingsCommand::SaveBootRom(rom_name.clone()));
                let _ = crate::SETTINGS_CHANNEL
                    .try_send(crate::SettingsCommand::LoadRomToMemory(rom_name.clone()));
                cursor = idx;
            }
            MenuEvent::Back => return PageState::Settings,
            MenuEvent::Home => return PageState::MainMenu,
            MenuEvent::Monitor => {
                return PageState::Monitor;
            }
            MenuEvent::LoadBlock(_) => {}
            MenuEvent::Timeout => {
                cursor = menu.selected_index();
            }
            _ => {}
        }
    }
}
