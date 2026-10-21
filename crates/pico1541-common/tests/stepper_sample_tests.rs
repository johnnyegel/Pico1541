use pico1541_common::sound::SoundVolume;
use pico1541_common::stepper_sample::{
    stepper_sample_to_pwm_duty, stepper_sample_to_pwm_duty_stopped, BUMP_SAMPLE,
    STEPPING_SAMPLE, STEPPER_DC_RAMP_SAMPLES, STEPPER_SAMPLE_RATE,
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
    let excursion_low = (stepper_sample_to_pwm_duty(20, top, SoundVolume::Low) as i32 - center as i32).abs();
    let excursion_med = (stepper_sample_to_pwm_duty(20, top, SoundVolume::Medium) as i32 - center as i32).abs();
    let excursion_high = (stepper_sample_to_pwm_duty(20, top, SoundVolume::High) as i32 - center as i32).abs();
    assert!(excursion_high > excursion_med);
    assert!(excursion_med > excursion_low);

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

