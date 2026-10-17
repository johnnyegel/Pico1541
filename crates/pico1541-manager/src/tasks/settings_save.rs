use crate::driver::SettingsDriver;
use crate::SettingsCommand;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;

pub struct SettingsSaveTask<D: SettingsDriver> {
    driver: D,
    rx: Receiver<'static, CriticalSectionRawMutex, SettingsCommand, 2>,
}

impl<D: SettingsDriver> SettingsSaveTask<D> {
    pub fn new(driver: D, rx: Receiver<'static, CriticalSectionRawMutex, SettingsCommand, 2>) -> Self {
        Self { driver, rx }
    }

    pub async fn run(&mut self) {
        loop {
            let cmd = self.rx.receive().await;

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
                        let inner_cmd = self.rx.receive().await;
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
                SettingsCommand::SaveSoundSettings(settings) => {
                    self.driver.save_sound_settings(&settings).await;
                }
                _ => {}
            }
        }
    }
}
