use crate::driver::IndicatorDriver;
use crate::{EVENT_CHANNEL, DriveEvent};

pub struct DriveActivityTask<I: IndicatorDriver> {
    indicator: I,
}

impl<I: IndicatorDriver> DriveActivityTask<I> {
    pub fn new(indicator: I) -> Self {
        Self { indicator }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = EVENT_CHANNEL.receive().await;
            match event {
                DriveEvent::LedChanged(active) => {
                    self.indicator.set_activity_led(active);
                }
                DriveEvent::HeadStepped { .. } => {
                    self.indicator.beep();
                }
                _ => {}
            }
        }
    }
}
