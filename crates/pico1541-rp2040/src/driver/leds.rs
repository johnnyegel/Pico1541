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
            red: Output::new(red, Level::High), // Active Low assumed
            green: Output::new(green, Level::High),
            blue: Output::new(blue, Level::High),
        }
    }
}

impl RgbLedDriver for StatusLeds {
    fn set_color(&mut self, r: bool, g: bool, b: bool) {
        self.red.set_level(if r { Level::Low } else { Level::High });
        self.green
            .set_level(if g { Level::Low } else { Level::High });
        self.blue
            .set_level(if b { Level::Low } else { Level::High });
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

    pub fn set_state(&mut self, active: bool) {
        // Example: Red when active, Blue/Off otherwise
        // Depends on the specific LED and wiring
        if active {
            self.pos.set_high();
            self.neg.set_low();
        } else {
            self.pos.set_low();
            self.neg.set_low();
        }
    }
}
