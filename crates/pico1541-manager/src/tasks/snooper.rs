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
                    self.handler.handle(event);
                    
                    // If this was an EOI, check if we have a complete command to process
                    if let DriveEvent::IecSnoopByteReceived(_, true) = event {
                        if let Some(cmd) = self.handler.get_command() {
                            let mut data = [0u8; 32];
                            let len = cmd.len().min(32) as u8;
                            data[..len as usize].copy_from_slice(&cmd.as_bytes()[..len as usize]);
                            
                            ACTIVITY_CHANNEL.send(DriveEvent::IecCommand { data, len }).await;
                        }
                    }
                }
                // Forward other events (LED, Motor, etc.) to the activity task
                other => {
                    ACTIVITY_CHANNEL.send(other).await;
                }
            }
        }
    }
}
