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

#[test]
fn test_bumped_not_cleared_when_steps_is_zero() {
    use pico1541_common::sound::SOUND_BUMPED_STOP;
    use portable_atomic::Ordering;

    let _ = get_and_clear_steps();

    // Simulate bump flag set right before step counter increment (or race condition window)
    SOUND_BUMPED_STOP.store(true, Ordering::Relaxed);

    // Core 0 polls while steps is still 0
    let (count, bumped) = get_and_clear_steps();
    assert_eq!(count, 0);
    assert!(!bumped);

    // Bumped flag must NOT have been cleared and lost!
    assert!(SOUND_BUMPED_STOP.load(Ordering::Relaxed));

    // Now Core 1 finishes incrementing step counter
    notify_step(false);

    let (count2, bumped2) = get_and_clear_steps();
    assert_eq!(count2, 1);
    assert!(bumped2);

    // Cleaned up
    let (count3, bumped3) = get_and_clear_steps();
    assert_eq!(count3, 0);
    assert!(!bumped3);
}
