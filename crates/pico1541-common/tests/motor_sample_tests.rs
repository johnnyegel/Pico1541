use pico1541_common::motor_sample::{sample_to_pwm_duty, MOTOR_HUM_SAMPLE, MOTOR_SAMPLE_RATE};
use pico1541_common::sound::SoundVolume;

#[test]
fn test_motor_sample_table_properties() {
    assert_eq!(MOTOR_HUM_SAMPLE.len(), 8819);
    assert_eq!(MOTOR_SAMPLE_RATE, 22050);

    // Verify all samples are within valid signed 8-bit range
    for &s in MOTOR_HUM_SAMPLE.iter() {
        assert!(s >= -60 && s <= 60, "Sample out of expected bounds: {}", s);
    }
}

#[test]
fn test_sample_to_pwm_duty_scaling() {
    let top = 11337u16; // 22,050 Hz @ 250 MHz
    let center = top / 2;

    // Off volume: duty should be 0 (no output, transistor turned off)
    assert_eq!(sample_to_pwm_duty(0, top, SoundVolume::Off), 0);
    assert_eq!(sample_to_pwm_duty(20, top, SoundVolume::Off), 0);
    assert_eq!(sample_to_pwm_duty(-20, top, SoundVolume::Off), 0);

    // Zero sample at Medium volume: duty should be centered at midpoint
    assert_eq!(sample_to_pwm_duty(0, top, SoundVolume::Medium), center);

    // Positive sample at Medium volume: duty should be above center but <= top
    let duty_pos = sample_to_pwm_duty(20, top, SoundVolume::Medium);
    assert!(duty_pos > center);
    assert!(duty_pos <= top);

    // Negative sample at Medium volume: duty should be below center but >= 0
    let duty_neg = sample_to_pwm_duty(-20, top, SoundVolume::Medium);
    assert!(duty_neg < center);

    // High volume should have greater excursion than Low volume
    let excursion_low = (sample_to_pwm_duty(20, top, SoundVolume::Low) as i32 - center as i32).abs();
    let excursion_high = (sample_to_pwm_duty(20, top, SoundVolume::High) as i32 - center as i32).abs();
    assert!(excursion_high > excursion_low);
}
