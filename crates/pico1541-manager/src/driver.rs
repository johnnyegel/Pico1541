use pico1541_common::Error;

/// Driver for block-based storage (e.g., SD Card).
pub trait StorageDriver {
    async fn read_block(&mut self, track: u8, sector: u8, buf: &mut [u8]) -> Result<(), Error>;
    async fn write_block(&mut self, track: u8, sector: u8, buf: &[u8]) -> Result<(), Error>;
}

/// Driver for the OLED/LCD display.
pub trait DisplayDriver {
    async fn clear(&mut self);
    async fn print(&mut self, line: u8, text: &str);
    async fn update(&mut self);
    async fn redraw(&mut self);
}

/// Driver for keyboard/button input.
pub trait InputDriver {
    async fn wait_for_button(&mut self) -> u8;
}

/// Driver for status and activity indicators.
pub trait IndicatorDriver {
    fn set_activity_led(&mut self, active: bool);
    fn beep(&mut self);
}
