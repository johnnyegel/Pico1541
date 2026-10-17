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
fn test_sound_settings_from_bytes_short_slices() {
    assert_eq!(SoundSettings::from_bytes(&[]), None);
    assert_eq!(SoundSettings::from_bytes(&[1]), None);
    assert_eq!(SoundSettings::from_bytes(&[1, 2]), None);
    assert!(SoundSettings::from_bytes(&[1, 1, 1]).is_some());
    assert!(SoundSettings::from_bytes(&[1, 1, 1, 99]).is_some());
}

#[test]
fn test_sound_volume_from_u8_and_duty_percent() {
    assert_eq!(SoundVolume::from_u8(0), SoundVolume::Off);
    assert_eq!(SoundVolume::from_u8(1), SoundVolume::Low);
    assert_eq!(SoundVolume::from_u8(2), SoundVolume::Medium);
    assert_eq!(SoundVolume::from_u8(3), SoundVolume::High);
    assert_eq!(SoundVolume::from_u8(4), SoundVolume::Off);
    assert_eq!(SoundVolume::from_u8(99), SoundVolume::Off);
    assert_eq!(SoundVolume::from_u8(255), SoundVolume::Off);

    assert_eq!(SoundVolume::Off.duty_percent(), 0);
    assert_eq!(SoundVolume::Low.duty_percent(), 5);
    assert_eq!(SoundVolume::Medium.duty_percent(), 20);
    assert_eq!(SoundVolume::High.duty_percent(), 50);
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

#[test]
fn test_sound_step_bumped_flag_persistence() {
    let _ = get_and_clear_steps();

    // Bumped first, then non-bumped step
    notify_step(true);
    notify_step(false);

    // Bumped flag must remain true until drained
    let (count, bumped) = get_and_clear_steps();
    assert_eq!(count, 2);
    assert!(bumped);

    // After draining, bumped flag is cleared
    let (count2, bumped2) = get_and_clear_steps();
    assert_eq!(count2, 0);
    assert!(!bumped2);
}
