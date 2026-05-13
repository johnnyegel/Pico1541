use crate::driver::DisplayDriver;
use crate::KEYBOARD_CHANNEL;
use crate::tasks::menu::Menu;
use embassy_time::{Duration, Timer};
use heapless::Vec;

pub struct MainMenuTask<D> {
    display: D,
}

impl<D: DisplayDriver> MainMenuTask<D> {
    pub fn new(display: D) -> Self {
        Self { display }
    }

    pub async fn run(&mut self) -> ! {
        // 1. Boot Screen
        self.display.clear().await;
        // Assuming line 1 for "Large" effect or center
        self.display.print(1, "   Pico1541   ").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_secs(2)).await;

        // 2. Main Menu Setup
        let mut items = Vec::<&'static str, 4>::new();
        let _ = items.push("SD Card");
        let _ = items.push("USB Drive");
        let _ = items.push("Settings");
        
        let mut menu = Menu::new("--- Main Menu ---", items);

        loop {
            self.display.clear().await;
            menu.render(&mut self.display).await;

            // Wait for keyboard input
            let buttons = KEYBOARD_CHANNEL.receive().await;
            
            // Bits: 3:Up, 1:Down, 2:Select, 0:Back
            if buttons & 0b1000 != 0 {
                menu.move_up();
            } else if buttons & 0b0010 != 0 {
                menu.move_down();
            } else if buttons & 0b0100 != 0 {
                let selected = menu.selected_item();
                log::info!("Selected: {}", selected);
                // Handle selection (next phase)
            } else if buttons & 0b0001 != 0 {
                log::info!("Back/Cancel pressed");
            }
        }
    }
}
