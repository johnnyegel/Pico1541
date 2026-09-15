use crate::driver::speaker::PiezoSpeaker;
use pico1541_manager::IndicatorDriver;

pub struct ActivityIndicator {
    pub speaker: PiezoSpeaker,
}

impl ActivityIndicator {
    pub fn new(speaker: PiezoSpeaker) -> Self {
        Self { speaker }
    }
}

impl IndicatorDriver for ActivityIndicator {
    fn beep(&mut self) {
        self.speaker.beep();
    }
}

pub struct IndicatorWrapper<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static>(
    pub &'static embassy_sync::mutex::Mutex<M, ActivityIndicator>,
);

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> Clone for IndicatorWrapper<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> Copy for IndicatorWrapper<M> {}

impl<M: embassy_sync::blocking_mutex::raw::RawMutex + 'static> IndicatorDriver
    for IndicatorWrapper<M>
{
    fn beep(&mut self) {
        if let Ok(mut i) = self.0.try_lock() {
            i.beep();
        }
    }
}
