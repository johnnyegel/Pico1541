use crate::{DriveEvent, ACTIVITY_CHANNEL, EVENT_CHANNEL};
use pico1541_common::snooper::{IecBusByte, IecSnooper, SnooperConfig};

pub struct SnooperTask {
    snooper: IecSnooper,
}

impl SnooperTask {
    pub fn new(device_id: u8, config: &'static SnooperConfig) -> Self {
        Self {
            snooper: IecSnooper::new(device_id, config),
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = EVENT_CHANNEL.receive().await;
            match event {
                DriveEvent::IecBusTransition(state, delta) => {
                    if let Some(cmd_data) = self.snooper.tick(state, delta) {
                        let len = self.snooper.buffer_len() as u8;
                        ACTIVITY_CHANNEL
                            .send(DriveEvent::IecCommand {
                                data: cmd_data,
                                len,
                            })
                            .await;
                    }
                }
                // Forward other events to the activity task
                other => {
                    ACTIVITY_CHANNEL.send(other).await;
                }
            }
        }
    }
}
