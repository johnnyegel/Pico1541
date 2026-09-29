use super::PageState;
use crate::driver::{DisplayDriver, FileStorageDriver};
use crate::tasks::menu::{Menu, MenuEvent};
use core::str::FromStr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::{String, Vec};
use pico1541_common::DriveLocation;

pub async fn run_main_menu<D, S, Internal>(
    display: &mut D,
    storage: &mut S,
    internal: &mut Internal,
    sd_path: &mut String<128>,
    location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
    context: &crate::tasks::ui::UiContext,
) -> PageState
where
    D: DisplayDriver,
    S: FileStorageDriver,
    Internal: FileStorageDriver,
{
    let mut cursor = 0;
    loop {
        let mut main_items = Vec::<String<16>, 4>::new();

        // 0. Internal
        let mut int_str = String::<16>::from_str("Internal").unwrap_or_default();
        let int_available = internal.is_available().await;
        if !int_available {
            let _ = int_str.push_str(" (X)");
        }
        let _ = main_items.push(int_str);

        // 1. SD Card
        let mut sd_str = String::<16>::from_str("SD Card").unwrap_or_default();
        let sd_available = storage.is_available().await;
        if !sd_available {
            let _ = sd_str.push_str(" (X)");
        }
        let _ = main_items.push(sd_str);

        // 2. Settings
        let _ = main_items.push(String::from_str("Settings").unwrap_or_default());

        let mut main_menu = Menu::new("--- Main Menu ---", main_items);
        main_menu.set_cursor(cursor);

        match main_menu.run(display, &context.action_rx).await {
            MenuEvent::Selected(idx) => {
                cursor = idx;
                match idx {
                    0 => {
                        if int_available {
                            let path = String::from_str("/files").unwrap_or_default();
                            let _ = context.cmd_tx.send(crate::InternalCommand::Mount(
                                crate::MountPayload::VirtualDirectory(path, true),
                            )).await;
                            return PageState::InternalMenu;
                        }
                    }
                    1 => {
                        if sd_available {
                            let path = String::from_str("/").unwrap_or_default();
                            let _ = context.cmd_tx.send(crate::InternalCommand::Mount(
                                crate::MountPayload::VirtualDirectory(path, false),
                            )).await;
                            return PageState::SdCardMenu;
                        }
                    }
                    2 => {
                        return PageState::Settings;
                    }
                    _ => {}
                }
            }
            MenuEvent::Back => {}
            MenuEvent::Home => {}
            MenuEvent::Monitor => {
                return PageState::Monitor;
            }
            MenuEvent::LoadBlock(_) => {}
            MenuEvent::Timeout => {
                cursor = main_menu.selected_index();
                if !storage.is_available().await {
                    *sd_path = String::from_str("/").unwrap_or_default();
                }
                // Loop continues to refresh status icons
            }
            MenuEvent::LocationChanged => {
                let loc = location.lock().await.clone();
                match loc {
                    DriveLocation::VirtualDirectory { use_internal, .. }
                    | DriveLocation::DiskImage { use_internal, .. } => {
                        return if use_internal {
                            PageState::InternalMenu
                        } else {
                            PageState::SdCardMenu
                        };
                    }
                    DriveLocation::Root => {}
                }
            }
            MenuEvent::ContextAction(_) => {}
        }
    }
}
