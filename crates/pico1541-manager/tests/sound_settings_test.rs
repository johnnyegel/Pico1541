use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico1541_common::sound::{SoundSettings, SoundVolume};
use pico1541_manager::driver::SettingsDriver;
use pico1541_manager::tasks::settings_save::SettingsSaveTask;
use pico1541_manager::SettingsCommand;

struct MockSettingsDriver {
    saved_sound: Option<SoundSettings>,
}

impl SettingsDriver for MockSettingsDriver {
    async fn save_drive_id(&mut self, _: u8) {}
    async fn save_boot_rom(&mut self, _: &str) {}
    async fn delete_rom(&mut self, _: &str) {}
    async fn install_rom_start(&mut self, _: &str, _: u32, _: u32) {}
    async fn install_rom_chunk(&mut self, _: &[u8]) {}
    async fn install_rom_finish(&mut self) {}
    async fn load_rom_to_memory(&mut self, _: &str) {}
    async fn save_sound_settings(&mut self, settings: &SoundSettings) {
        self.saved_sound = Some(*settings);
    }
}

#[test]
fn test_settings_save_sound() {
    pollster::block_on(async {
        let mut driver = MockSettingsDriver { saved_sound: None };
        let target = SoundSettings {
            volume: SoundVolume::High,
            motor_enabled: true,
            stepper_enabled: false,
        };
        driver.save_sound_settings(&target).await;
        assert_eq!(driver.saved_sound, Some(target));
    });
}

struct ObservableMockDriver {
    saved_sound: std::sync::Arc<std::sync::Mutex<Option<SoundSettings>>>,
}

impl SettingsDriver for ObservableMockDriver {
    async fn save_drive_id(&mut self, _: u8) {}
    async fn save_boot_rom(&mut self, _: &str) {}
    async fn delete_rom(&mut self, _: &str) {}
    async fn install_rom_start(&mut self, _: &str, _: u32, _: u32) {}
    async fn install_rom_chunk(&mut self, _: &[u8]) {}
    async fn install_rom_finish(&mut self) {}
    async fn load_rom_to_memory(&mut self, _: &str) {}
    async fn save_sound_settings(&mut self, settings: &SoundSettings) {
        *self.saved_sound.lock().unwrap() = Some(*settings);
    }
}

#[test]
fn test_settings_save_task_dispatches_save_sound_settings() {
    pollster::block_on(async {
        static CHANNEL: Channel<CriticalSectionRawMutex, SettingsCommand, 2> = Channel::new();
        let saved_sound = std::sync::Arc::new(std::sync::Mutex::new(None));
        let driver = ObservableMockDriver {
            saved_sound: saved_sound.clone(),
        };
        let mut task = SettingsSaveTask::new(driver, CHANNEL.receiver());

        let target = SoundSettings {
            volume: SoundVolume::Low,
            motor_enabled: false,
            stepper_enabled: true,
        };

        let task_runner = task.run();
        let test_logic = async {
            CHANNEL.send(SettingsCommand::SaveSoundSettings(target)).await;
            while saved_sound.lock().unwrap().is_none() {
                embassy_futures::yield_now().await;
            }
            assert_eq!(*saved_sound.lock().unwrap(), Some(target));
        };

        match select(task_runner, test_logic).await {
            Either::First(_) => panic!("Task exited unexpectedly"),
            Either::Second(_) => {}
        }
    });
}
