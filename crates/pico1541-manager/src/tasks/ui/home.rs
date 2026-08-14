use heapless::{String, Vec};
use core::str::FromStr;
use crate::driver::{DisplayDriver, FileStorageDriver};
use crate::tasks::menu::{Menu, MenuEvent};
use super::PageState;

pub async fn run_main_menu<D, S, Internal>(
    display: &mut D,
    storage: &mut S,
    internal: &mut Internal,
    sd_path: &mut String<128>,
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
        let mut int_str = String::<16>::from_str("Internal").unwrap();
        let int_available = internal.is_available().await;
        if !int_available {
            let _ = int_str.push_str(" (X)");
        }
        let _ = main_items.push(int_str);

        // 1. SD Card
        let mut sd_str = String::<16>::from_str("SD Card").unwrap();
        let sd_available = storage.is_available().await;
        if !sd_available {
            let _ = sd_str.push_str(" (X)");
        }
        let _ = main_items.push(sd_str);

        // 2. Settings
        let _ = main_items.push(String::from_str("Settings").unwrap());
        
        let mut main_menu = Menu::new("--- Main Menu ---", main_items);
        main_menu.set_cursor(cursor);

        match main_menu.run(display).await {
            MenuEvent::Selected(idx) => {
                cursor = idx;
                match idx {
                    0 => {
                        if int_available {
                            return PageState::InternalMenu;
                        }
                    }
                    1 => {
                        if sd_available {
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
                    *sd_path = String::from_str("/").unwrap();
                }
                // Loop continues to refresh status icons
            }
            MenuEvent::ContextAction(_) => {}
        }
    }
}
