use crate::driver::IndicatorDriver;
use crate::{DriveEvent, EVENT_CHANNEL};
use log::*;

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
                DriveEvent::IecCommand { data, len } => {
                    let cmd_str =
                        core::str::from_utf8(&data[..len as usize]).unwrap_or("INVALID UTF8");
                    info!("Manager received IEC command: {}", cmd_str);
                    self.handle_command(cmd_str).await;
                }
                _ => {}
            }
        }
    }

    async fn handle_command(&mut self, cmd: &str) {
        if cmd.starts_with("CD:") {
            let path = &cmd[3..];
            info!("Changing directory to: {}", path);
            // In a real implementation, this would interact with the filesystem driver
        } else if cmd.starts_with("MD:") {
            let path = &cmd[3..];
            info!("Creating directory: {}", path);
        }
    }
}
