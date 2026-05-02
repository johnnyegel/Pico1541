use crate::driver::leds::ActivityLed;
use crate::driver::speaker::PiezoSpeaker;
use pico1541_manager::IndicatorDriver;

pub struct ActivityIndicator {
    pub activity_led: ActivityLed,
    pub speaker: PiezoSpeaker,
}

impl ActivityIndicator {
    pub fn new(activity_led: ActivityLed, speaker: PiezoSpeaker) -> Self {
        Self {
            activity_led,
            speaker,
        }
    }
}

impl IndicatorDriver for ActivityIndicator {
    fn set_activity_led(&mut self, active: bool) {
        self.activity_led.set_state(active);
    }

    fn beep(&mut self) {
        self.speaker.beep();
    }
}
