use embassy_rp::pwm::{ChannelAPin, Config as PwmConfig, Pwm, Slice};
use embassy_rp::Peri;

pub struct PwmPiezo<'d> {
    pwm: Pwm<'d>,
}

impl<'d> PwmPiezo<'d> {
    pub fn new<T: Slice>(
        slice: Peri<'d, T>,
        pin: Peri<'d, impl ChannelAPin<T>>,
    ) -> Self {
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        let pwm = Pwm::new_output_a(slice, pin, config);
        Self { pwm }
    }

    pub fn stop(&mut self) {
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        self.pwm.set_config(&config);
    }

    /// Set frequency in Hz and duty cycle percent (0-50%)
    pub fn play_tone(&mut self, freq_hz: u32, duty_percent: u8) {
        if freq_hz == 0 || duty_percent == 0 {
            self.stop();
            return;
        }

        // System clock is 250 MHz or 125 MHz. Use divider 64 to fit 16-bit TOP.
        // For 110 Hz: 250_000_000 / (64 * 110) = 35_511 (fits in u16!)
        let divider: u8 = 64;
        let counter_hz = 250_000_000 / (divider as u32);
        let top = (counter_hz / freq_hz).clamp(1, 65535) as u16;
        let compare = (top as u32 * (duty_percent as u32) / 100).clamp(0, top as u32) as u16;

        let mut config = PwmConfig::default();
        config.divider = divider.into();
        config.top = top;
        config.compare_a = compare;
        self.pwm.set_config(&config);
    }
}
