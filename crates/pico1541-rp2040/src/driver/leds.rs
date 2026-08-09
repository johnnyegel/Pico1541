use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::Peri;
use pico1541_manager::driver::{RgbLedDriver, StatusLedMode};

/// Driver for the RGB Status/Power LED (GPIO 26, 27, 28).
pub struct StatusLeds {
    pub red: Output<'static>,
    pub green: Output<'static>,
    pub blue: Output<'static>,
}

impl StatusLeds {
    pub fn new(
        red: Peri<'static, impl Pin>,
        green: Peri<'static, impl Pin>,
        blue: Peri<'static, impl Pin>,
    ) -> Self {
        Self {
            red: Output::new(red, Level::Low),
            green: Output::new(green, Level::Low),
            blue: Output::new(blue, Level::Low),
        }
    }
}

impl RgbLedDriver for StatusLeds {
    fn set_color(&mut self, r: bool, g: bool, b: bool) {
        self.red.set_level(if r { Level::High } else { Level::Low });
        self.green
            .set_level(if g { Level::High } else { Level::Low });
        self.blue
            .set_level(if b { Level::High } else { Level::Low });
    }

    fn set_status(&mut self, mode: StatusLedMode) {
        let (r, g, b) = match mode {
            StatusLedMode::Unmounted => (true, false, false),      // Red
            StatusLedMode::VirtualDisk => (false, false, true),    // Blue
            StatusLedMode::NonAccurateDisk => (true, true, false),  // Yellow (Red + Green)
            StatusLedMode::AccurateDisk => (false, true, false),   // Green
        };
        self.set_color(r, g, b);
    }
}

pub struct StatusLedsWrapper<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static>(
    pub &'static embassy_sync::mutex::Mutex<M, StatusLeds>,
);

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> Clone for StatusLedsWrapper<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> Copy for StatusLedsWrapper<M> {}

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> RgbLedDriver for StatusLedsWrapper<M> {
    fn set_color(&mut self, red: bool, green: bool, blue: bool) {
        if let Ok(mut l) = self.0.try_lock() {
            l.set_color(red, green, blue);
        }
    }

    fn set_status(&mut self, mode: StatusLedMode) {
        if let Ok(mut l) = self.0.try_lock() {
            l.set_status(mode);
        }
    }
}

