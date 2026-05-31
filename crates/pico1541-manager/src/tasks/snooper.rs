use crate::{DriveEvent, ACTIVITY_CHANNEL, EVENT_CHANNEL};
use pico1541_common::snooper::IecSnoopByteHandler;

pub struct SnooperTask {
    handler: IecSnoopByteHandler,
}

impl SnooperTask {
    pub fn new(_device_id: u8) -> Self {
        Self {
            handler: IecSnoopByteHandler::new(),
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = EVENT_CHANNEL.receive().await;
            match event {
                DriveEvent::IecSnoopSecondaryAddress(_) | DriveEvent::IecSnoopByteReceived(_, _) => {
                    self.handler.handle(event, &mut |e| {
                        let _ = ACTIVITY_CHANNEL.try_send(e);
                    });
                }
                // Forward other events (LED, Motor, etc.) to the activity task
                other => {
                    ACTIVITY_CHANNEL.send(other).await;
                }
            }
        }
    }
}
