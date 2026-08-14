use crate::formats::Error;

pub mod file_storage;

/// Driver for block-based storage (e.g., SD Card).
pub trait StorageDriver {
    async fn read_block(&mut self, track: u8, sector: u8, buf: &mut [u8]) -> Result<(), Error>;
    async fn write_block(&mut self, track: u8, sector: u8, buf: &[u8]) -> Result<(), Error>;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FileInfo {
    pub name: [u8; 64], // support long file names up to 63 chars + null
    pub size: u32,
    pub is_dir: bool,
}
pub trait FileStorageDriver {
    type File: embedded_io_async::Read + embedded_io_async::Seek + 'static;

    async fn is_available(&self) -> bool;
    async fn list_dir(&mut self, path: &str, callback: impl FnMut(FileInfo)) -> Result<(), Error>;
    async fn open_file(&mut self, path: &str) -> Result<(), Error>;

    fn open_file_handle(&self, path: &str) -> Self::File;

    fn volume_label(&self) -> Option<heapless::String<11>> {
        None
    }

    async fn free_space(&self) -> Option<u32> {
        None
    }

    async fn list_dir_range(
        &mut self,
        path: &str,
        start_index: usize,
        count: usize,
        mut callback: impl FnMut(FileInfo),
    ) -> Result<(), Error> {
        let mut idx = 0;
        self.list_dir(path, |info| {
            if idx >= start_index && idx < start_index + count {
                callback(info);
            }
            idx += 1;
        }).await
    }
}/// Driver for the OLED/LCD display.
pub trait DisplayDriver {
    async fn clear(&mut self);
    async fn print(&mut self, line: u8, text: &str);
    async fn print_large(&mut self, line: u8, text: &str);
    async fn update(&mut self);
    async fn redraw(&mut self);
    fn request_redraw(&mut self);
    fn is_dirty(&self) -> bool;
    fn set_selected_line(&mut self, _line: u8) {}
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
    async fn wait_for_change(&mut self);
    async fn scan(&mut self) -> u8;
}

/// Driver for status and activity indicators.
pub trait IndicatorDriver {
    fn beep(&mut self);
}

impl<T: IndicatorDriver + ?Sized> IndicatorDriver for &mut T {
    fn beep(&mut self) {
        (**self).beep();
    }
}
