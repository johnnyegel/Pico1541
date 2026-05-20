use pico1541_common::formats::Error;

pub mod internal_storage;

pub use internal_storage::InternalStorage;

/// Driver for block-based storage (e.g., SD Card).
pub trait StorageDriver {
    async fn read_block(&mut self, track: u8, sector: u8, buf: &mut [u8]) -> Result<(), Error>;
    async fn write_block(&mut self, track: u8, sector: u8, buf: &[u8]) -> Result<(), Error>;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FileInfo {
    pub name: [u8; 13], // 8.3 format + null
    pub size: u32,
    pub is_dir: bool,
}

pub trait FileStorageDriver {
    type File: embedded_io_async::Read + embedded_io_async::Seek + 'static;

    async fn is_available(&self) -> bool;
    async fn list_dir(&mut self, path: &str, callback: impl FnMut(FileInfo)) -> Result<(), Error>;
    async fn open_file(&mut self, path: &str) -> Result<(), Error>;

    fn open_file_handle(&self, path: &str) -> Self::File;
}

/// Driver for the OLED/LCD display.
pub trait DisplayDriver {
    async fn clear(&mut self);
    async fn print(&mut self, line: u8, text: &str);
    async fn print_large(&mut self, line: u8, text: &str);
    async fn update(&mut self);
    async fn redraw(&mut self);
    fn request_redraw(&mut self);
    fn is_dirty(&self) -> bool;
}

/// Status LED mode representing the disk mounting state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusLedMode {
    /// Nothing is mounted.
    Unmounted,
    /// A virtual image (aka directory) is mounted.
    VirtualDisk,
    /// A non-accurate disk image (e.g. D64) is mounted.
    NonAccurateDisk,
    /// An accurate disk image (e.g. G64) is mounted.
    AccurateDisk,
}

/// Driver for RGB LED control.
pub trait RgbLedDriver {
    fn set_color(&mut self, r: bool, g: bool, b: bool);
    fn set_status(&mut self, mode: StatusLedMode);
}

impl<T: RgbLedDriver + ?Sized> RgbLedDriver for &mut T {
    fn set_color(&mut self, r: bool, g: bool, b: bool) {
        (**self).set_color(r, g, b);
    }

    fn set_status(&mut self, mode: StatusLedMode) {
        (**self).set_status(mode);
    }
}

/// Driver for keyboard/button input.
pub trait InputDriver {
    async fn wait_for_button(&mut self) -> u8;
    fn scan(&mut self) -> u8;
}

/// Color for the bi-color Activity LED.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityColor {
    Off,
    Red,
    Green,
}

/// Driver for status and activity indicators.
pub trait IndicatorDriver {
    fn set_activity_led(&mut self, color: ActivityColor);
    fn beep(&mut self);
}

impl<T: IndicatorDriver + ?Sized> IndicatorDriver for &mut T {
    fn set_activity_led(&mut self, color: ActivityColor) {
        (**self).set_activity_led(color);
    }

    fn beep(&mut self) {
        (**self).beep();
    }
}
