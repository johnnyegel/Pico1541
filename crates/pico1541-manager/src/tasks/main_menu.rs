use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver, FileStorageDriver, FileInfo, InternalStorage};
use crate::KEYBOARD_CHANNEL;
use crate::tasks::menu::{Menu, MenuEvent};
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

        loop {
            let mut main_items = Vec::<String<16>, 4>::new();
            let mut item_map = Vec::<usize, 4>::new();

            if self.storage.is_available().await {
                let _ = main_items.push(String::from_str("SD Card").unwrap());
                let _ = item_map.push(0);
            }

            if self.internal.is_available().await {
                let _ = main_items.push(String::from_str("Internal").unwrap());
                let _ = item_map.push(1);
            }

            // USB Drive (Future)
            // let _ = main_items.push(String::from_str("USB Drive").unwrap());
            // let _ = item_map.push(2);

            let _ = main_items.push(String::from_str("Settings").unwrap());
            let _ = item_map.push(3);
            
            let mut main_menu = Menu::new("--- Main Menu ---", main_items);

            match main_menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    match item_map[idx] {
                        0 => self.sd_card_menu().await, // SD Card
                        1 => self.internal_menu().await, // Internal Storage
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
        let _ = items.push("ROMs");
        let _ = items.push("Drive ID");
        let _ = items.push("Hardware Test");
        
        let mut menu = Menu::new("--- Settings ---", items);

        loop {
            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    match idx {
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
}
