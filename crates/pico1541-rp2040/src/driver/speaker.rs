use embassy_rp::gpio::{Level, Output, AnyPin};

pub struct PiezoSpeaker {
    pub piezo: Output<'static>,
}

impl PiezoSpeaker {
    pub fn new(pin: AnyPin) -> Self {
        Self {
            piezo: Output::new(pin, Level::Low),
        }
    }

    pub fn beep(&mut self) {
        // Simple toggle for now, proper PWM/PIQ later
        self.piezo.set_high();
    }
}
