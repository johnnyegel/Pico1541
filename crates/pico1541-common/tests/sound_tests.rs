use pico1541_common::sound::{
    get_and_clear_steps, is_motor_running, notify_motor, notify_step, SoundSettings, SoundVolume,
};

#[test]
fn test_sound_settings_default_and_roundtrip() {
    let default = SoundSettings::default();
    assert_eq!(default.volume, SoundVolume::Medium);
    assert!(default.motor_enabled);
    assert!(default.stepper_enabled);

    let bytes = default.to_bytes();
    let restored = SoundSettings::from_bytes(&bytes).expect("deserialization failed");
    assert_eq!(default, restored);

    let custom = SoundSettings {
        volume: SoundVolume::Off,
        motor_enabled: false,
        stepper_enabled: true,
    };
    let restored_custom = SoundSettings::from_bytes(&custom.to_bytes()).unwrap();
    assert_eq!(custom, restored_custom);
}

#[test]
fn test_sound_notification_atomics() {
    let _ = get_and_clear_steps();
    notify_motor(false);
    assert!(!is_motor_running());

    notify_motor(true);
    assert!(is_motor_running());

    notify_step(false);
    notify_step(true);

    let (count, bumped) = get_and_clear_steps();
    assert_eq!(count, 2);
    assert!(bumped);

    let (count2, bumped2) = get_and_clear_steps();
    assert_eq!(count2, 0);
    assert!(!bumped2);
}
