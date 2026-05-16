use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver, FileStorageDriver, FileInfo, InternalStorage};
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

pub struct MainMenuTask<D, L, Ind, S, Disk: 'static> {
    display: D,
    leds: L,
    indicator: Ind,
    storage: S,
    internal: InternalStorage,
    disk_image: &'static Mutex<CriticalSectionRawMutex, Disk>,
    config: &'static SnooperConfig,
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
        Self { display, leds, indicator, storage, internal: InternalStorage, disk_image, config }
    }

    pub async fn run(&mut self) -> ! {
        // 1. Boot Screen
        self.display.clear().await;
        self.display.print_large(1, " Pico1541 ").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_secs(2)).await;

        let mut cursor = 0;
        loop {
            let mut main_items = Vec::<String<16>, 4>::new();
            
            // 0. SD Card
            let mut sd_str = String::<16>::from_str("SD Card").unwrap();
            let sd_available = self.storage.is_available().await;
            if !sd_available {
                let _ = sd_str.push_str(" (X)");
            }
            let _ = main_items.push(sd_str);

            // 1. Internal
            let mut int_str = String::<16>::from_str("Internal").unwrap();
            let int_available = self.internal.is_available().await;
            if !int_available {
                let _ = int_str.push_str(" (X)");
            }
            let _ = main_items.push(int_str);

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
                            if sd_available {
                                self.sd_card_menu().await;
                            }
                        }
                        1 => {
                            if int_available {
                                self.internal_menu().await;
                            }
                        }
                        3 => self.settings_menu().await, // Settings
                        _ => {
                            log::info!("Selected: {}", main_menu.selected_item());
                        }
                    }
                }
                MenuEvent::Back => {
                    // Back in main menu could maybe show a "Shutting down" or just ignore
                }
            }
        }
    }

    async fn sd_card_menu(&mut self) {
        self.file_browser_menu("SD Card", false).await;
    }

    async fn internal_menu(&mut self) {
        self.file_browser_menu("Internal", true).await;
    }

    async fn file_browser_menu(&mut self, title: &str, use_internal: bool) {
        let mut file_infos = Vec::<FileInfo, 16>::new();
        
        if use_internal {
            let _ = self.internal.list_dir("/", |info| {
                let _ = file_infos.push(info);
            }).await;
        } else {
            let _ = self.storage.list_dir("/", |info| {
                let _ = file_infos.push(info);
            }).await;
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
            let _ = menu.run(&mut self.display).await;
            return;
        }

        let mut menu = Menu::new(menu_title, items);

        loop {
            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    let selected = &file_infos[idx];
                    if !selected.is_dir {
                        log::info!("Selected file: {:?}", selected);
                        self.display.clear().await;
                        self.display.print(1, "   Mounting...  ").await;
                        self.display.request_redraw();
                        
                        // Actually mounting would happen here.
                        // For now we just show a message.
                        Timer::after(Duration::from_secs(1)).await;
                        return;
                    }
                }
                MenuEvent::Back => return,
            }
        }
    }

    async fn settings_menu(&mut self) {
        let mut items = Vec::<&'static str, 4>::new();
        let _ = items.push("Drive ID");
        let _ = items.push("Monitor");
        let _ = items.push("Hardware Test");
        
        let mut menu = Menu::new("--- Settings ---", items);

        loop {
            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    match idx {
                        0 => self.device_id_menu().await,
                        1 => self.monitor_page().await,
                        2 => {
                            // Hardware Test
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
                MenuEvent::Back => return,
            }
        }
    }

    async fn monitor_page(&mut self) {
        loop {
            self.display.clear().await;
            self.display.print(0, "--- Monitor ---").await;

            let speed_khz = self.config.emu_speed_khz.load(Ordering::Relaxed);
            let jitter_ns = self.config.emu_jitter_ns.load(Ordering::Relaxed);

            let mut s_speed = String::<32>::new();
            let _ = core::fmt::write(&mut s_speed, format_args!("Speed: {:>4} kHz", speed_khz));
            self.display.print(2, &s_speed).await;

            let mut s_jitter = String::<32>::new();
            let _ = core::fmt::write(&mut s_jitter, format_args!("Jitter: {:>8} ns", jitter_ns));
            self.display.print(3, &s_jitter).await;

            self.display.print(5, " [Back] ").await;
            self.display.request_redraw();

            // Refresh every 500ms or on button press
            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(500),
                crate::KEYBOARD_CHANNEL.receive()
            ).await {
                Ok(ButtonEvent::Pressed(Button::Back)) => return,
                _ => {}
            }
        }
    }

    async fn device_id_menu(&mut self) {
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
                MenuEvent::Back => return,
            }
        }
    }
}
