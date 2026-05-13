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
    fn set_activity_led(&mut self, color: pico1541_manager::driver::ActivityColor) {
        self.activity_led.set_color(color);
    }

    fn beep(&mut self) {
        self.speaker.beep();
    }
}

pub struct IndicatorWrapper<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static>(
    pub &'static embassy_sync::mutex::Mutex<M, ActivityIndicator>,
);

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> IndicatorDriver
    for IndicatorWrapper<M>
{
    fn set_activity_led(&mut self, color: pico1541_manager::driver::ActivityColor) {
        if let Ok(mut i) = self.0.try_lock() {
            i.set_activity_led(color);
        }
    }

    fn beep(&mut self) {
        if let Ok(mut i) = self.0.try_lock() {
            i.beep();
        }
    }
}

