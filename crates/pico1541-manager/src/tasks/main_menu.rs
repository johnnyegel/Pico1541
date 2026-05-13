use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver, FileStorageDriver, FileInfo, InternalStorage};
use crate::KEYBOARD_CHANNEL;
use crate::tasks::menu::Menu;
use crate::tasks::hardware_test::HardwareTestTask;
use embassy_time::{Duration, Timer};
use heapless::{Vec, String};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use core::str::FromStr;

pub struct MainMenuTask<D, L, Ind, S, Disk: 'static> {
    display: D,
    leds: L,
    indicator: Ind,
    storage: S,
    internal: InternalStorage,
    disk_image: &'static Mutex<CriticalSectionRawMutex, Disk>,
}

impl<D, L, Ind, S, Disk> MainMenuTask<D, L, Ind, S, Disk>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver + Clone,
    Disk: 'static,
{
    pub fn new(display: D, leds: L, indicator: Ind, storage: S, disk_image: &'static Mutex<CriticalSectionRawMutex, Disk>) -> Self {
        Self { display, leds, indicator, storage, internal: InternalStorage, disk_image }
    }

    pub async fn run(&mut self) -> ! {
        // 1. Boot Screen
        self.display.clear().await;
        self.display.print_large(1, " Pico1541 ").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_secs(2)).await;

        let mut main_items = Vec::<&'static str, 4>::new();
        let _ = main_items.push("SD Card");
        let _ = main_items.push("Internal");
        let _ = main_items.push("USB Drive");
        let _ = main_items.push("Settings");
        
        let mut main_menu = Menu::new("--- Main Menu ---", main_items);

        loop {
            self.display.clear().await;
            main_menu.render(&mut self.display).await;

            let buttons = KEYBOARD_CHANNEL.receive().await;
            
            if buttons & 0b1000 != 0 {
                main_menu.move_up();
            } else if buttons & 0b0010 != 0 {
                main_menu.move_down();
            } else if buttons & 0b0100 != 0 {
                match main_menu.selected_index() {
                    0 => self.sd_card_menu().await, // SD Card
                    1 => self.internal_menu().await, // Internal Storage
                    3 => self.settings_menu().await, // Settings
                    _ => {
                        log::info!("Selected: {}", main_menu.selected_item());
                    }
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
            let menu = Menu::new(menu_title, empty_items);
            self.display.clear().await;
            menu.render(&mut self.display).await;
            loop {
                let buttons = KEYBOARD_CHANNEL.receive().await;
                if buttons & 0b0001 != 0 { return; }
            }
        }

        let mut menu = Menu::new(menu_title, items);

        loop {
            self.display.clear().await;
            menu.render(&mut self.display).await;

            let buttons = KEYBOARD_CHANNEL.receive().await;
            
            if buttons & 0b1000 != 0 {
                menu.move_up();
            } else if buttons & 0b0010 != 0 {
                menu.move_down();
            } else if buttons & 0b0100 != 0 {
                let selected = &file_infos[menu.selected_index()];
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
            } else if buttons & 0b0001 != 0 {
                return;
            }
        }
    }

    async fn settings_menu(&mut self) {
        let mut items = Vec::<&'static str, 4>::new();
        let _ = items.push("ROMs");
        let _ = items.push("Drive ID");
        let _ = items.push("Hardware Test");
        
        let mut menu = Menu::new("--- Settings ---", items);

        loop {
            self.display.clear().await;
            menu.render(&mut self.display).await;

            let buttons = KEYBOARD_CHANNEL.receive().await;
            
            if buttons & 0b1000 != 0 {
                menu.move_up();
            } else if buttons & 0b0010 != 0 {
                menu.move_down();
            } else if buttons & 0b0100 != 0 {
                match menu.selected_index() {
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
            } else if buttons & 0b0001 != 0 {
                // Back to Main Menu
                return;
            }
        }
    }
}
