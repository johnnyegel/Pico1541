use pico1541_manager::DisplayDriver;

pub struct RpDisplay {
    // TODO: Add I2C handle for GPIO 4, 5
}

impl RpDisplay {
    pub fn new() -> Self {
        Self {}
    }
}

impl DisplayDriver for RpDisplay {
    async fn clear(&mut self) {
        // TODO: Implement OLED clear
    }
    async fn print(&mut self, _line: u8, _text: &str) {
        // TODO: Implement OLED print
    }
}
