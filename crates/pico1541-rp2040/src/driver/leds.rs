use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::Peri;
use pico1541_manager::driver::RgbLedDriver;

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
}

/// Driver for the Activity LED (Bi-color, GPIO 22, 3).
pub struct ActivityLed {
    pub pos: Output<'static>,
    pub neg: Output<'static>,
}

impl ActivityLed {
    pub fn new(
        pos: Peri<'static, impl Pin>,
        neg: Peri<'static, impl Pin>,
    ) -> Self {
        Self {
            pos: Output::new(pos, Level::Low),
            neg: Output::new(neg, Level::Low),
        }
    }

    pub fn set_color(&mut self, color: pico1541_manager::driver::ActivityColor) {
        use pico1541_manager::driver::ActivityColor;
        match color {
            ActivityColor::Red => {
                self.pos.set_low();
                self.neg.set_high();
            }
            ActivityColor::Green => {
                self.pos.set_high();
                self.neg.set_low();
            }
            ActivityColor::Off => {
                self.pos.set_low();
                self.neg.set_low();
            }
        }
    }
}
