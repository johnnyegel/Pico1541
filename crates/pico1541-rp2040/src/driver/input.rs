use embassy_time::{Duration, Timer};
use pico1541_manager::InputDriver;

#[allow(dead_code)]
pub struct KeyboardInput {
    // TODO: Add GPIOs for Keyboard Matrix 6, 7, 8, 9
}

#[allow(dead_code)]
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
