use pico1541_manager::InputDriver;
use embassy_time::{Duration, Timer};

pub struct KeyboardInput {
    // TODO: Add GPIOs for Keyboard Matrix 6, 7, 8, 9
}

impl KeyboardInput {
    pub fn new() -> Self {
        Self {}
    }
}

impl InputDriver for KeyboardInput {
    async fn wait_for_button(&mut self) -> u8 {
        // TODO: Implement button wait using Matrix
        Timer::after(Duration::from_secs(1)).await;
        0
    }
}
