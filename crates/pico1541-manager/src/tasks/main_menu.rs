use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver};
use crate::KEYBOARD_CHANNEL;
use crate::tasks::menu::Menu;
use crate::tasks::hardware_test::HardwareTestTask;
use embassy_time::{Duration, Timer};
use heapless::Vec;

pub struct MainMenuTask<D, L, Ind> {
    display: D,
    leds: L,
    indicator: Ind,
}

impl<D: DisplayDriver + Clone, L: RgbLedDriver + Clone, Ind: IndicatorDriver + Clone> MainMenuTask<D, L, Ind> {
    pub fn new(display: D, leds: L, indicator: Ind) -> Self {
        Self { display, leds, indicator }
    }

    pub async fn run(&mut self) -> ! {
        // 1. Boot Screen
        self.display.clear().await;
        self.display.print(1, "   Pico1541   ").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_secs(2)).await;

        let mut main_items = Vec::<&'static str, 4>::new();
        let _ = main_items.push("SD Card");
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
                    2 => self.settings_menu().await, // Settings
                    _ => {
                        log::info!("Selected: {}", main_menu.selected_item());
                    }
                }
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
