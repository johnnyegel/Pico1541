use crate::driver::IndicatorDriver;
use crate::{EVENT_CHANNEL, DriveEvent};

pub struct ActivityTask<I: IndicatorDriver> {
    indicator: I,
}

impl<I: IndicatorDriver> ActivityTask<I> {
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
