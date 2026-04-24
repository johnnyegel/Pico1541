use pico1541_manager::IndicatorDriver;
use crate::driver::leds::ActivityLed;
use crate::driver::speaker::RpSpeaker;

pub struct RpIndicator {
    pub activity_led: ActivityLed,
    pub speaker: RpSpeaker,
}

impl RpIndicator {
    pub fn new(activity_led: ActivityLed, speaker: RpSpeaker) -> Self {
        Self {
            activity_led,
            speaker,
        }
    }
}

impl IndicatorDriver for RpIndicator {
    fn set_activity_led(&mut self, active: bool) {
        self.activity_led.set_state(active);
    }

    fn beep(&mut self) {
        self.speaker.beep();
    }
}
