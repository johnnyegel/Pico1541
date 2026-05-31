use crate::driver::IndicatorDriver;
use crate::{DriveEvent, ACTIVITY_CHANNEL};
use log::*;

pub struct DriveActivityTask<I: IndicatorDriver> {
    indicator: I,
    led_on: bool,
    motor_on: bool,
}

impl<I: IndicatorDriver> DriveActivityTask<I> {
    pub fn new(indicator: I) -> Self {
        Self { 
            indicator,
            led_on: false,
            motor_on: false,
        }
    }

    fn update_led(&mut self) {
        let color = if self.led_on {
            crate::driver::ActivityColor::Red
        } else if self.motor_on {
            crate::driver::ActivityColor::Green
        } else {
            crate::driver::ActivityColor::Off
        };
        self.indicator.set_activity_led(color);
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = ACTIVITY_CHANNEL.receive().await;
            match event {
                DriveEvent::LedChanged(active) => {
                    self.led_on = active;
                    self.update_led();
                }
                DriveEvent::MotorChanged(active) => {
                    self.motor_on = active;
                    self.update_led();
                    if active {
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ResetDirectory);
                    }
                }
                DriveEvent::HeadStepped { new_half_track } => {
                    self.indicator.beep();
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::HeadStepped(new_half_track));
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
