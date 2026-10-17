use pico1541_common::sound::{SoundSettings, SoundVolume};

#[test]
fn test_sound_settings_cycle_volume() {
    let mut vol = SoundVolume::Off;
    vol = match vol {
        SoundVolume::Off => SoundVolume::Low,
        SoundVolume::Low => SoundVolume::Medium,
        SoundVolume::Medium => SoundVolume::High,
        SoundVolume::High => SoundVolume::Off,
    };
    assert_eq!(vol, SoundVolume::Low);
    vol = match vol {
        SoundVolume::Off => SoundVolume::Low,
        SoundVolume::Low => SoundVolume::Medium,
        SoundVolume::Medium => SoundVolume::High,
        SoundVolume::High => SoundVolume::Off,
    };
    assert_eq!(vol, SoundVolume::Medium);
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
