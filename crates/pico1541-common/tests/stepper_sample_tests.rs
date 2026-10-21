use pico1541_common::sound::SoundVolume;
use pico1541_common::stepper_sample::{
    mixed_sample_to_pwm_duty_faded, stepper_sample_to_pwm_duty,
    stepper_sample_to_pwm_duty_stopped, BUMP_SAMPLE, STEPPING_SAMPLE, STEPPER_DC_RAMP_SAMPLES,
    STEPPER_SAMPLE_RATE,
};

#[test]
fn test_stepper_sample_table_properties() {
    assert_eq!(STEPPING_SAMPLE.len(), 691);
    assert_eq!(BUMP_SAMPLE.len(), 2616);
    assert_eq!(STEPPER_SAMPLE_RATE, 22050);
    assert_eq!(STEPPER_DC_RAMP_SAMPLES, 25);

    // Verify first and last elements match source
    assert_eq!(STEPPING_SAMPLE[0], -2);
    assert_eq!(STEPPING_SAMPLE[690], 4);
    assert_eq!(BUMP_SAMPLE[0], 3);
    assert_eq!(BUMP_SAMPLE[2615], -3);

    // Verify all samples are within valid range and exact extrema match
    let min_step = *STEPPING_SAMPLE.iter().min().unwrap();
    let max_step = *STEPPING_SAMPLE.iter().max().unwrap();
    assert_eq!(min_step, -28);
    assert_eq!(max_step, 31);

    let min_bump = *BUMP_SAMPLE.iter().min().unwrap();
    let max_bump = *BUMP_SAMPLE.iter().max().unwrap();
    assert_eq!(min_bump, -101);
    assert_eq!(max_bump, 90);
}


#[test]
fn test_stepper_sample_to_pwm_duty() {
    let top = 11337u16; // 22,050 Hz @ 250 MHz
    let center = top / 2;

    // Off volume: duty should be 0 (no output, transistor turned off)
    assert_eq!(stepper_sample_to_pwm_duty(0, top, SoundVolume::Off), 0);
    assert_eq!(stepper_sample_to_pwm_duty(20, top, SoundVolume::Off), 0);
    assert_eq!(stepper_sample_to_pwm_duty(-20, top, SoundVolume::Off), 0);

    // Zero sample at Medium volume: duty should be centered at midpoint
    assert_eq!(stepper_sample_to_pwm_duty(0, top, SoundVolume::Medium), center);

    // Positive sample at Medium volume: duty should be above center but <= top
    let duty_pos = stepper_sample_to_pwm_duty(20, top, SoundVolume::Medium);
    assert!(duty_pos > center);
    assert!(duty_pos <= top);

    // Negative sample at Medium volume: duty should be below center but >= 0
    let duty_neg = stepper_sample_to_pwm_duty(-20, top, SoundVolume::Medium);
    assert!(duty_neg < center);

    // High volume should have greater excursion than Medium, and Medium > Low
    let excursion_low = (stepper_sample_to_pwm_duty(10, top, SoundVolume::Low) as i32 - center as i32).abs();
    let excursion_med = (stepper_sample_to_pwm_duty(10, top, SoundVolume::Medium) as i32 - center as i32).abs();
    let excursion_high = (stepper_sample_to_pwm_duty(10, top, SoundVolume::High) as i32 - center as i32).abs();
    assert!(excursion_high > excursion_med);
    assert!(excursion_med > excursion_low);

    // Calibrated excursion checks at TOP = 11337 with sample = 10:
    // Low: top / 120 = 94 -> 10 * 94 = 940
    // Med: top / 50 = 226 -> 10 * 226 = 2260
    // High: top / 30 = 377 -> 10 * 377 = 3770
    assert_eq!(excursion_low, 940);
    assert_eq!(excursion_med, 2260);
    assert_eq!(excursion_high, 3770);

    // Extreme values should clamp within [0, top]
    assert!(stepper_sample_to_pwm_duty(127, top, SoundVolume::High) <= top);
    assert!(stepper_sample_to_pwm_duty(-128, top, SoundVolume::High) <= top);
}

#[test]
fn test_stepper_sample_to_pwm_duty_stopped_ramping() {
    let top = 11337u16;
    let total_len = STEPPING_SAMPLE.len();

    // Off volume must always produce 0
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(20, top, SoundVolume::Off, 0, total_len),
        0
    );
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(20, top, SoundVolume::Off, 100, total_len),
        0
    );

    // Index 0: scale is 0, so duty should be 0 (starts at 0V DC)
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, 0, total_len),
        0
    );
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(20, top, SoundVolume::High, 0, total_len),
        0
    );

    // Last index (total_len - 1): remaining = 0, scale is 0, so duty should be 0 (ends at 0V DC)
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, total_len - 1, total_len),
        0
    );
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(20, top, SoundVolume::High, total_len - 1, total_len),
        0
    );

    // Mid-section (steady state): should match active duty
    for idx in [50, 100, 200, 300, 500] {
        for sample in [-25i8, 0, 25] {
            assert_eq!(
                stepper_sample_to_pwm_duty_stopped(sample, top, SoundVolume::High, idx, total_len),
                stepper_sample_to_pwm_duty(sample, top, SoundVolume::High)
            );
        }
    }

    // Ramp up DC bias monotonicity (sample = 0)
    let mut prev_dc = 0;
    for idx in 0..STEPPER_DC_RAMP_SAMPLES {
        let dc = stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, idx, total_len);
        assert!(dc >= prev_dc, "Ramp up DC bias must be non-decreasing: {} vs {}", dc, prev_dc);
        prev_dc = dc;
    }

    // Ramp down DC bias monotonicity (sample = 0)
    let mut prev_dc = top;
    for idx in (total_len - STEPPER_DC_RAMP_SAMPLES)..total_len {
        let dc = stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, idx, total_len);
        assert!(dc <= prev_dc, "Ramp down DC bias must be non-increasing: {} vs {}", dc, prev_dc);
        prev_dc = dc;
    }

    // Verify bump sample stopped duty ramping boundaries
    let bump_len = BUMP_SAMPLE.len();
    assert_eq!(stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, 0, bump_len), 0);
    assert_eq!(stepper_sample_to_pwm_duty_stopped(0, top, SoundVolume::High, bump_len - 1, bump_len), 0);
    let bump_mid = bump_len / 2;
    assert_eq!(
        stepper_sample_to_pwm_duty_stopped(BUMP_SAMPLE[bump_mid], top, SoundVolume::High, bump_mid, bump_len),
        stepper_sample_to_pwm_duty(BUMP_SAMPLE[bump_mid], top, SoundVolume::High)
    );
}

#[test]
fn test_mixed_sample_to_pwm_duty_faded() {
    let top = 11337u16;
    let center = top / 2;

    // Volume Off or step 0 produces 0
    assert_eq!(mixed_sample_to_pwm_duty_faded(10, 20, top, SoundVolume::Off, 15, 15), 0);
    assert_eq!(mixed_sample_to_pwm_duty_faded(10, 20, top, SoundVolume::High, 0, 15), 0);

    // Zero samples at steady state (step 15/15) produces center
    assert_eq!(mixed_sample_to_pwm_duty_faded(0, 0, top, SoundVolume::High, 15, 15), center);
    assert_eq!(mixed_sample_to_pwm_duty_faded(0, 0, top, SoundVolume::Medium, 15, 15), center);
    assert_eq!(mixed_sample_to_pwm_duty_faded(0, 0, top, SoundVolume::Low, 15, 15), center);

    // Sum of AC excursions
    let duty_both_pos = mixed_sample_to_pwm_duty_faded(10, 10, top, SoundVolume::High, 15, 15);
    let duty_motor_only = mixed_sample_to_pwm_duty_faded(10, 0, top, SoundVolume::High, 15, 15);
    let duty_step_only = mixed_sample_to_pwm_duty_faded(0, 10, top, SoundVolume::High, 15, 15);
    assert!(duty_both_pos > duty_motor_only);
    assert!(duty_both_pos > duty_step_only);
    assert!(duty_both_pos <= top);

    // Clamping limits
    let duty_max_clamp = mixed_sample_to_pwm_duty_faded(127, 127, top, SoundVolume::High, 15, 15);
    assert_eq!(duty_max_clamp, top);
    let duty_min_clamp = mixed_sample_to_pwm_duty_faded(-128, -128, top, SoundVolume::High, 15, 15);
    assert_eq!(duty_min_clamp, 0);

    // Zero output on max_steps = 0
    assert_eq!(mixed_sample_to_pwm_duty_faded(10, 20, top, SoundVolume::High, 5, 0), 0);

    // Overrun clamping: step > max_steps clamps to max_steps
    assert_eq!(
        mixed_sample_to_pwm_duty_faded(20, 20, top, SoundVolume::High, 25, 15),
        mixed_sample_to_pwm_duty_faded(20, 20, top, SoundVolume::High, 15, 15)
    );

    // Destructive AC interference
    let duty_opposing = mixed_sample_to_pwm_duty_faded(10, -10, top, SoundVolume::High, 15, 15);
    assert!(duty_opposing < duty_both_pos);

    // Envelope scaling at mid-step (e.g. 7 of 15)
    let duty_mid_dc = mixed_sample_to_pwm_duty_faded(0, 0, top, SoundVolume::High, 7, 15);
    let expected_center = ((top as u32 / 2) * 7 / 15) as u16;
    assert_eq!(duty_mid_dc, expected_center);
}


