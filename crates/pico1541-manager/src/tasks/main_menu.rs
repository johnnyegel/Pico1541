use crate::driver::{DisplayDriver, RgbLedDriver, StatusLedMode, IndicatorDriver, FileStorageDriver, FileInfo, InternalStorage};
use crate::tasks::menu::{Menu, MenuEvent};
use crate::tasks::hardware_test::HardwareTestTask;
use crate::{Button, ButtonEvent};
use embassy_time::{Duration, Timer};
use heapless::{Vec, String};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use core::str::FromStr;
use pico1541_common::snooper::SnooperConfig;
use portable_atomic::Ordering;

fn parent_dir(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    let p = if path.ends_with('/') && path.len() > 1 {
        &path[..path.len() - 1]
    } else {
        path
    };
    if let Some(idx) = p.rfind('/') {
        if idx == 0 {
            "/"
        } else {
            &p[..idx]
        }
    } else {
        "/"
    }
}

fn join_path(current: &str, sub: &str) -> String<128> {
    let mut new_path = String::<128>::new();
    if current == "/" {
        let _ = new_path.push_str("/");
        let _ = new_path.push_str(sub);
    } else {
        let _ = new_path.push_str(current);
        let _ = new_path.push_str("/");
        let _ = new_path.push_str(sub);
    }
    new_path
}

pub struct MainMenuTask<D, L, Ind, S, Disk: 'static> {
    display: D,
    leds: L,
    indicator: Ind,
    storage: S,
    internal: InternalStorage,
    _disk_image: &'static Mutex<CriticalSectionRawMutex, Disk>,
    config: &'static SnooperConfig,
    sd_path: String<128>,
    internal_path: String<128>,
    usb_path: String<128>,
}

impl<D, L, Ind, S, Disk> MainMenuTask<D, L, Ind, S, Disk>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver + Clone,
    Disk: 'static,
{
    pub fn new(display: D, leds: L, indicator: Ind, storage: S, disk_image: &'static Mutex<CriticalSectionRawMutex, Disk>, config: &'static SnooperConfig) -> Self {
        Self {
            display,
            leds,
            indicator,
            storage,
            internal: InternalStorage,
            _disk_image: disk_image,
            config,
            sd_path: String::from_str("/").unwrap(),
            internal_path: String::from_str("/").unwrap(),
            usb_path: String::from_str("/").unwrap(),
        }
    }

    pub async fn run(&mut self) -> ! {
        // Initialize the status LED to the default startup mode (VirtualDisk)
        self.leds.set_status(StatusLedMode::VirtualDisk);

        // 1. Boot Screen
        self.display.clear().await;
        self.display.print_large(2, "  Pico1541").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_millis(1500)).await;

        let mut cursor = 0;
        loop {
            let mut main_items = Vec::<String<16>, 4>::new();

            // 0. Internal
            let mut int_str = String::<16>::from_str("Internal").unwrap();
            let int_available = self.internal.is_available().await;
            if !int_available {
                let _ = int_str.push_str(" (X)");
            }
            let _ = main_items.push(int_str);

            // 1. SD Card
            let mut sd_str = String::<16>::from_str("SD Card").unwrap();
            let sd_available = self.storage.is_available().await;
            if !sd_available {
                let _ = sd_str.push_str(" (X)");
            }
            let _ = main_items.push(sd_str);

            // 2. USB Drive (Not yet supported)
            let mut usb_str = String::<16>::from_str("USB Drive").unwrap();
            let _ = usb_str.push_str(" (X)");
            let _ = main_items.push(usb_str);

            // 3. Settings
            let _ = main_items.push(String::from_str("Settings").unwrap());
            
            let mut main_menu = Menu::new("--- Main Menu ---", main_items);
            main_menu.set_cursor(cursor);

            match main_menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    cursor = idx;
                    match idx {
                        0 => {
                            if int_available {
                                let _ = self.internal_menu().await;
                            }
                        }
                        1 => {
                            if sd_available {
                                let _ = self.sd_card_menu().await;
                            }
                        }
                        3 => {
                            let _ = self.settings_menu().await;
                        }
                        _ => {
                            log::info!("Selected: {}", main_menu.selected_item());
                        }
                    }
                }
                MenuEvent::Back => {
                    // Back in main menu could maybe show a "Shutting down" or just ignore
                }
                MenuEvent::Home => {
                    // Home in main menu does nothing
                }
            }
        }
    }

    async fn sd_card_menu(&mut self) -> bool {
        self.file_browser_menu("SD Card", false).await
    }

    async fn internal_menu(&mut self) -> bool {
        self.file_browser_menu("Internal", true).await
    }

    async fn file_browser_menu(&mut self, title: &str, use_internal: bool) -> bool {
        let mut cursor = 0;
        loop {
            let current_path = if use_internal {
                &self.internal_path
            } else if title == "SD Card" {
                &self.sd_path
            } else {
                &self.usb_path
            };

            let mut file_infos = Vec::<FileInfo, 16>::new();
            
            let list_res = if use_internal {
                self.internal.list_dir(current_path.as_str(), |info| {
                    let _ = file_infos.push(info);
                }).await
            } else {
                self.storage.list_dir(current_path.as_str(), |info| {
                    let _ = file_infos.push(info);
                }).await
            };

            if let Err(e) = list_res {
                log::error!("Error listing dir: {:?}", e);
                // Reset path on error
                let current_path_mut = if use_internal {
                    &mut self.internal_path
                } else if title == "SD Card" {
                    &mut self.sd_path
                } else {
                    &mut self.usb_path
                };
                *current_path_mut = String::from_str("/").unwrap();

                self.display.clear().await;
                self.display.print(1, "  Read Error!").await;
                self.display.request_redraw();
                Timer::after(Duration::from_secs(2)).await;
                return false;
            }

            let mut items = Vec::<String<16>, 16>::new();
            for info in file_infos.iter() {
                let mut s = String::<16>::new();
                let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                let _ = s.push_str(name_str.trim_matches('\0'));
                if info.is_dir {
                    let _ = s.push_str("/");
                }
                let _ = items.push(s);
            }

            let mut menu_title = String::<32>::from_str("--- ").unwrap();
            let _ = menu_title.push_str(title);
            let _ = menu_title.push_str(" ---");

            if items.is_empty() {
                let mut empty_items = Vec::<&'static str, 1>::new();
                let _ = empty_items.push("No files found");
                let mut menu = Menu::new(menu_title, empty_items);
                match menu.run(&mut self.display).await {
                    MenuEvent::Back => {
                        let current_path_mut = if use_internal {
                            &mut self.internal_path
                        } else if title == "SD Card" {
                            &mut self.sd_path
                        } else {
                            &mut self.usb_path
                        };
                        if current_path_mut.as_str() == "/" {
                            return false;
                        } else {
                            let parent = parent_dir(current_path_mut.as_str());
                            *current_path_mut = String::from_str(parent).unwrap();
                            cursor = 0;
                            continue;
                        }
                    }
                    MenuEvent::Home => return true,
                    _ => {}
                }
                return false;
            }

            let mut menu = Menu::new(menu_title, items);
            menu.set_cursor(cursor.min(file_infos.len() - 1));

            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    let selected = &file_infos[idx];
                    if selected.is_dir {
                        let dirname = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let current_path_mut = if use_internal {
                            &mut self.internal_path
                        } else if title == "SD Card" {
                            &mut self.sd_path
                        } else {
                            &mut self.usb_path
                        };
                        let new_path = join_path(current_path_mut.as_str(), dirname);
                        *current_path_mut = new_path;
                        cursor = 0;
                    } else {
                        log::info!("Selected file: {:?}", selected);
                        self.display.clear().await;
                        self.display.print(1, "   Mounting...  ").await;
                        self.display.request_redraw();

                        let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let mut full_path = String::<128>::new();
                        let current_path_val = if use_internal {
                            self.internal_path.as_str()
                        } else if title == "SD Card" {
                            self.sd_path.as_str()
                        } else {
                            self.usb_path.as_str()
                        };
                        if current_path_val == "/" {
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        } else {
                            let _ = full_path.push_str(current_path_val);
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        }

                        if use_internal {
                            let _ = self.internal.open_file(full_path.as_str()).await;
                        } else {
                            let _ = self.storage.open_file(full_path.as_str()).await;
                        }
                        Timer::after(Duration::from_secs(1)).await;
                        return false;
                    }
                }
                MenuEvent::Back => {
                    let current_path_mut = if use_internal {
                        &mut self.internal_path
                    } else if title == "SD Card" {
                        &mut self.sd_path
                    } else {
                        &mut self.usb_path
                    };
                    if current_path_mut.as_str() == "/" {
                        return false;
                    } else {
                        let parent = parent_dir(current_path_mut.as_str());
                        *current_path_mut = String::from_str(parent).unwrap();
                        cursor = 0;
                    }
                }
                MenuEvent::Home => return true,
            }
        }
    }

    async fn settings_menu(&mut self) -> bool {
        let mut items = Vec::<&'static str, 4>::new();
        let _ = items.push("Drive ID");
        let _ = items.push("Monitor");
        let _ = items.push("Hardware Test");
        
        let mut menu = Menu::new("--- Settings ---", items);

        loop {
            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    match idx {
                        0 => {
                            if self.device_id_menu().await {
                                return true;
                            }
                        }
                        1 => {
                            if self.monitor_page().await {
                                return true;
                            }
                        }
                        2 => {
                            let mut test = HardwareTestTask::new(
                                self.display.clone(),
                                self.leds.clone(),
                                self.indicator.clone(),
                            );
                            test.run().await;
                        }
                        _ => {
                            log::info!("Selected setting: {}", menu.selected_item());
                        }
                    }
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
            }
        }
    }

    async fn monitor_page(&mut self) -> bool {
        loop {
            self.display.clear().await;
            self.display.print(0, "-- System Monitor --").await;

            let speed_hz = self.config.emu_speed_hz.load(Ordering::Relaxed);
            let c0_avg = self.config.core0_avg_raw.load(Ordering::Relaxed);
            let c0_max = self.config.core0_max_raw.load(Ordering::Relaxed);
            let c1_avg = self.config.core1_avg_raw.load(Ordering::Relaxed);
            let c1_max = self.config.core1_max_raw.load(Ordering::Relaxed);

            let mut s_speed = String::<32>::new();
            if speed_hz >= 1_000_000 {
                let mhz_int = speed_hz / 1_000_000;
                let mhz_frac = (speed_hz % 1_000_000) / 10_000;
                let _ = core::fmt::write(&mut s_speed, format_args!("CPU Speed: {}.{:02} MHz", mhz_int, mhz_frac));
            } else {
                let _ = core::fmt::write(&mut s_speed, format_args!("CPU Speed: {} Hz", speed_hz));
            }
            self.display.print(2, &s_speed).await;

            let c0_load = if c0_max > 0 {
                100u32.saturating_sub((c0_avg * 100) / c0_max)
            } else {
                0
            };
            let mut s_c0 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c0, format_args!("Core 0 Load: {}%", c0_load));
            self.display.print(3, &s_c0).await;

            let c1_load = if c1_max > 0 {
                100u32.saturating_sub((c1_avg * 100) / c1_max)
            } else {
                0
            };
            let mut s_c1 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c1, format_args!("Core 1 Load: {}%", c1_load));
            self.display.print(4, &s_c1).await;

            self.display.print(6, " [Back] ").await;
            self.display.request_redraw();

            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(500),
                crate::KEYBOARD_CHANNEL.receive()
            ).await {
                Ok(ButtonEvent::Pressed(Button::Back)) => return false,
                Ok(ButtonEvent::LongPress(Button::Back)) => return true,
                _ => {}
            }
        }
    }

    async fn device_id_menu(&mut self) -> bool {
        let mut cursor = (self.config.device_id.load(portable_atomic::Ordering::Relaxed) - 8) as usize;
        loop {
            let current_id = self.config.device_id.load(portable_atomic::Ordering::Relaxed);
            let mut items = Vec::<String<16>, 4>::new();
            
            for id in 8..=11 {
                let mut s = String::<16>::new();
                if id == current_id {
                    let _ = s.push_str("* ");
                } else {
                    let _ = s.push_str("  ");
                }
                let _ = core::fmt::write(&mut s, format_args!("{}", id));
                let _ = items.push(s);
            }

            let mut menu = Menu::new("--- Drive ID ---", items);
            menu.set_cursor(cursor);

            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    cursor = idx;
                    let new_id = (idx + 8) as u8;
                    self.config.update_device_id(new_id);
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
            }
        }
    }
}
