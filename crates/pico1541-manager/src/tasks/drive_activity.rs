use crate::driver::IndicatorDriver;
use crate::{DriveEvent, ACTIVITY_CHANNEL};
use log::*;

pub struct DriveActivityTask<I: IndicatorDriver> {
    _indicator: I,
}

impl<I: IndicatorDriver> DriveActivityTask<I> {
    pub fn new(indicator: I) -> Self {
        Self {
            _indicator: indicator,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = ACTIVITY_CHANNEL.receive().await;
            match event {
                DriveEvent::IecCommand { data, len } => {
                    let cmd_str =
                        core::str::from_utf8(&data[..len as usize]).unwrap_or("INVALID UTF8");
                    info!("Manager received IEC command: {}", cmd_str);
                    self.handle_command(cmd_str).await;
                }
                DriveEvent::IecCloseChannel(sa) => {
                    if sa == 0 {
                        info!("IEC channel 0 closed.");
                        let _ =
                            crate::CMD_CHANNEL.try_send(crate::InternalCommand::CloseChannel(sa));
                    }
                }
                DriveEvent::IecBusIdleTimeout => {
                    info!("IEC bus idle timeout (>250ms). Rewinding directory.");
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::CloseChannel(0));
                }
                _ => {}
            }
        }
    }

    async fn handle_command(&mut self, cmd: &str) {
        if cmd.starts_with('$') {
            info!("Directory list requested via IEC: {}", cmd);
            let mut filename = [0xA0; 16];
            let len = cmd.len().min(16);
            filename[..len].copy_from_slice(&cmd.as_bytes()[..len]);
            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::LoadFile(filename));
        } else if let Some(path) = cmd.strip_prefix("CD:") {
            info!("Changing directory to: {}", path);

            let mut filename = [0xA0; 16];
            let len = path.len().min(16);
            filename[..len].copy_from_slice(&path.as_bytes()[..len]);

            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::LoadFile(filename));
        } else if let Some(path) = cmd.strip_prefix("MD:") {
            info!("Creating directory: {}", path);
        }
    }
}
