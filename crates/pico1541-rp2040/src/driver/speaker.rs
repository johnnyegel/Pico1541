use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::Peri;

pub struct PiezoSpeaker {
    pub piezo: Output<'static>,
}

impl PiezoSpeaker {
    pub fn new(pin: Peri<'static, impl Pin>) -> Self {
        Self {
            piezo: Output::new(pin, Level::Low),
        }
    }

    pub fn beep(&mut self) {
        // Simple toggle for now, proper PWM/PIQ later
        self.piezo.set_high();
    }
}
