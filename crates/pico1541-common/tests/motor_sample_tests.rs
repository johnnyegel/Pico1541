use pico1541_common::motor_sample::{
    sample_to_pwm_duty, sample_to_pwm_duty_faded, MOTOR_HUM_SAMPLE, MOTOR_SAMPLE_RATE,
};
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

#[test]
fn test_sample_to_pwm_duty_faded_boundaries() {
    let top = 11337u16;

    // Step 0 should always produce duty = 0 (0V DC)
    for sample in [-128i8, -25, 0, 25, 127] {
        assert_eq!(sample_to_pwm_duty_faded(sample, top, SoundVolume::High, 0, 15), 0);
        assert_eq!(sample_to_pwm_duty_faded(sample, top, SoundVolume::Medium, 0, 15), 0);
        assert_eq!(sample_to_pwm_duty_faded(sample, top, SoundVolume::Low, 0, 15), 0);
    }

    // Step max_steps should match sample_to_pwm_duty
    for sample in [-25i8, 0, 25] {
        assert_eq!(
            sample_to_pwm_duty_faded(sample, top, SoundVolume::High, 15, 15),
            sample_to_pwm_duty(sample, top, SoundVolume::High)
        );
        assert_eq!(
            sample_to_pwm_duty_faded(sample, top, SoundVolume::Medium, 15, 15),
            sample_to_pwm_duty(sample, top, SoundVolume::Medium)
        );
    }

    // Mid-step (e.g. step 7 of 15) must scale both DC center and AC amplitude smoothly
    let duty_mid_zero = sample_to_pwm_duty_faded(0, top, SoundVolume::High, 7, 15);
    let expected_center = ((top as u32 / 2) * 7 / 15) as u16;
    assert_eq!(duty_mid_zero, expected_center);

    // Monotonic DC center progression
    let mut prev_dc = 0;
    for step in 0..=15 {
        let dc = sample_to_pwm_duty_faded(0, top, SoundVolume::High, step, 15);
        assert!(dc >= prev_dc, "DC bias must be monotonically non-decreasing");
        prev_dc = dc;
    }

    // Defensive edge cases:
    // 1. max_steps == 0 must return 0 and not panic with divide by zero
    assert_eq!(sample_to_pwm_duty_faded(20, top, SoundVolume::High, 5, 0), 0);

    // 2. step > max_steps must clamp to max_steps behavior
    assert_eq!(
        sample_to_pwm_duty_faded(20, top, SoundVolume::High, 20, 15),
        sample_to_pwm_duty(20, top, SoundVolume::High)
    );

    // 3. SoundVolume::Off must always return 0 regardless of step
    assert_eq!(sample_to_pwm_duty_faded(20, top, SoundVolume::Off, 10, 15), 0);
}

