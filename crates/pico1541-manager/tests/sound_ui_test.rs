use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use pico1541_common::sound::{SoundSettings, SoundVolume};
use pico1541_manager::driver::{DisplayDriver, FileStorageDriver};
use pico1541_manager::tasks::ui::settings::run_sound_settings_menu;
use pico1541_manager::tasks::ui::{PageState, UiContext};
use pico1541_manager::{InternalCommand, SettingsCommand, UiAction};

#[no_mangle]
fn _embassy_time_now() -> u64 {
    0
}

#[no_mangle]
fn _embassy_time_schedule_wake(_at: u64, _waker: &core::task::Waker) {}

struct MockDisplay;

impl DisplayDriver for MockDisplay {
    async fn clear(&mut self) {}
    async fn print(&mut self, _line: u8, _text: &str) {}
    async fn print_large(&mut self, _line: u8, _text: &str) {}
    async fn update(&mut self) {}
    async fn redraw(&mut self) {}
    fn request_redraw(&mut self) {}
    fn is_dirty(&self) -> bool {
        false
    }
}

struct MockStorage;

impl FileStorageDriver for MockStorage {
    type File = pico1541_manager::storage::MemoryStorage<'static>;

    async fn is_available(&self) -> bool {
        true
    }

    async fn list_dir(
        &mut self,
        _path: &str,
        _callback: impl FnMut(pico1541_manager::driver::FileInfo),
    ) -> Result<(), pico1541_manager::formats::Error> {
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), pico1541_manager::formats::Error> {
        Ok(())
    }

    fn open_file_handle(&self, _path: &str) -> Self::File {
        pico1541_manager::storage::MemoryStorage::new(&[])
    }
}

#[test]
fn test_sound_volume_cycle_all_transitions() {
    assert_eq!(SoundVolume::Off.cycle(), SoundVolume::Low);
    assert_eq!(SoundVolume::Low.cycle(), SoundVolume::Medium);
    assert_eq!(SoundVolume::Medium.cycle(), SoundVolume::High);
    assert_eq!(SoundVolume::High.cycle(), SoundVolume::Off);
}

#[test]
fn test_sound_settings_defaults_and_toggle() {
    let mut settings = SoundSettings::default();
    assert_eq!(settings.volume, SoundVolume::Medium);
    assert!(settings.motor_enabled);
    assert!(settings.stepper_enabled);

    settings.motor_enabled = !settings.motor_enabled;
    assert!(!settings.motor_enabled);

    settings.stepper_enabled = !settings.stepper_enabled;
    assert!(!settings.stepper_enabled);
}

#[test]
fn test_run_sound_settings_menu_cycle_volume() {
    let mut display = MockDisplay;
    let mut storage = MockStorage;
    let action_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let cmd_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, InternalCommand, 32>::new()));
    let settings_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, SettingsCommand, 8>::new()));

    let context = UiContext {
        action_rx: action_channel.receiver(),
        cmd_tx: cmd_channel.sender(),
        settings_tx: settings_channel.sender(),
    };

    let mut sound_settings = SoundSettings::default();
    assert_eq!(sound_settings.volume, SoundVolume::Medium);

    // Select volume (cycles Medium -> High), then Back
    let _ = action_channel.sender().try_send(UiAction::Select);
    let _ = action_channel.sender().try_send(UiAction::Back);

    let next_state = block_on(run_sound_settings_menu(
        &mut display,
        &mut storage,
        &context,
        &mut sound_settings,
    ));

    assert_eq!(next_state, PageState::Settings);
    assert_eq!(sound_settings.volume, SoundVolume::High);

    // Verify SettingsCommand was sent
    let sent_cmd = settings_channel.receiver().try_receive();
    assert_eq!(
        sent_cmd,
        Ok(SettingsCommand::SaveSoundSettings(sound_settings))
    );
}

#[test]
fn test_run_sound_settings_menu_toggle_motor_and_stepper() {
    let mut display = MockDisplay;
    let mut storage = MockStorage;
    let action_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let cmd_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, InternalCommand, 32>::new()));
    let settings_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, SettingsCommand, 8>::new()));

    let context = UiContext {
        action_rx: action_channel.receiver(),
        cmd_tx: cmd_channel.sender(),
        settings_tx: settings_channel.sender(),
    };

    let mut sound_settings = SoundSettings::default();

    // Scroll to Motor, toggle Motor (true -> false), drain settings channel
    let _ = action_channel.sender().try_send(UiAction::ScrollDown);
    let _ = action_channel.sender().try_send(UiAction::Select);
    // Scroll to Stepper, toggle Stepper (true -> false)
    let _ = action_channel.sender().try_send(UiAction::ScrollDown);
    let _ = action_channel.sender().try_send(UiAction::Select);
    // Scroll to "Back" item and select it (idx 3)
    let _ = action_channel.sender().try_send(UiAction::ScrollDown);
    let _ = action_channel.sender().try_send(UiAction::Select);

    let next_state = block_on(run_sound_settings_menu(
        &mut display,
        &mut storage,
        &context,
        &mut sound_settings,
    ));

    assert_eq!(next_state, PageState::Settings);
    assert!(!sound_settings.motor_enabled);
    assert!(!sound_settings.stepper_enabled);
}

#[test]
fn test_run_sound_settings_menu_monitor_shortcut() {
    let mut display = MockDisplay;
    let mut storage = MockStorage;
    let action_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let cmd_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, InternalCommand, 32>::new()));
    let settings_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, SettingsCommand, 8>::new()));

    let context = UiContext {
        action_rx: action_channel.receiver(),
        cmd_tx: cmd_channel.sender(),
        settings_tx: settings_channel.sender(),
    };

    let mut sound_settings = SoundSettings::default();

    let _ = action_channel.sender().try_send(UiAction::Monitor);

    let next_state = block_on(run_sound_settings_menu(
        &mut display,
        &mut storage,
        &context,
        &mut sound_settings,
    ));

    assert_eq!(next_state, PageState::Monitor);
}

#[test]
fn test_run_sound_settings_menu_home_shortcut() {
    let mut display = MockDisplay;
    let mut storage = MockStorage;
    let action_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let cmd_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, InternalCommand, 32>::new()));
    let settings_channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, SettingsCommand, 8>::new()));

    let context = UiContext {
        action_rx: action_channel.receiver(),
        cmd_tx: cmd_channel.sender(),
        settings_tx: settings_channel.sender(),
    };

    let mut sound_settings = SoundSettings::default();

    let _ = action_channel.sender().try_send(UiAction::Home);

    let next_state = block_on(run_sound_settings_menu(
        &mut display,
        &mut storage,
        &context,
        &mut sound_settings,
    ));

    assert_eq!(next_state, PageState::MainMenu);
}
