use embassy_rp::gpio::{AnyPin, Level, Output};

#[allow(dead_code)]
/// Driver for the RGB Status/Power LED (GPIO 26, 27, 28).
pub struct StatusLeds {
    pub red: Output<'static>,
    pub green: Output<'static>,
    pub blue: Output<'static>,
}

#[allow(dead_code)]
impl StatusLeds {
    pub fn new(red: AnyPin, green: AnyPin, blue: AnyPin) -> Self {
        Self {
            red: Output::new(red, Level::High), // Active Low assumed
            green: Output::new(green, Level::High),
            blue: Output::new(blue, Level::High),
        }
    }

    pub fn set_color(&mut self, r: bool, g: bool, b: bool) {
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
    pub fn new(pos: AnyPin, neg: AnyPin) -> Self {
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
