use crate::driver::SettingsDriver;
use crate::{SettingsCommand, SETTINGS_CHANNEL};

pub struct SettingsSaveTask<D: SettingsDriver> {
    driver: D,
}

impl<D: SettingsDriver> SettingsSaveTask<D> {
    pub fn new(driver: D) -> Self {
        Self { driver }
    }

    pub async fn run(&mut self) {
        loop {
            let cmd = SETTINGS_CHANNEL.receive().await;

            match cmd {
                SettingsCommand::SaveDriveId(id) => {
                    self.driver.save_drive_id(id).await;
                }
                SettingsCommand::SaveBootRom(name) => {
                    self.driver.save_boot_rom(name.as_str()).await;
                }
                SettingsCommand::DeleteRom(name) => {
                    self.driver.delete_rom(name.as_str()).await;
                }
                SettingsCommand::InstallRomStart {
                    filename,
                    size,
                    offset,
                } => {
                    self.driver
                        .install_rom_start(filename.as_str(), size, offset)
                        .await;

                    loop {
                        let inner_cmd = SETTINGS_CHANNEL.receive().await;
                        match inner_cmd {
                            SettingsCommand::InstallRomChunk(data) => {
                                self.driver.install_rom_chunk(&data).await;
                            }
                            SettingsCommand::InstallRomFinish => {
                                self.driver.install_rom_finish().await;
                                break;
                            }
                            _ => {
                                // Ignore other commands during installation
                            }
                        }
                    }
                }
                SettingsCommand::LoadRomToMemory(name) => {
                    self.driver.load_rom_to_memory(name.as_str()).await;
                }
                _ => {}
            }
        }
    }
}
