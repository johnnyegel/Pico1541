# Authentic 1541 Audio Gain Rebalance Design

## 1. Background & Problem Statement
On real Commodore 1541 disk drives, the spindle motor hum is a gentle background rotary vibration, while the head stepping motor produces distinct, crisp mechanical snaps as the head carriage steps between tracks, and loud clattering thuds during head bumps at track 0.

During hardware testing of the continuous-phase audio mixing on the RP2350 with the piezo transducer:
- The spindle motor hum at the default `Medium` volume had an excursion of $\pm 2,825$ counts ($\approx 50\%$ peak-to-peak PWM swing at carrier $22,050\text{ Hz}$), which continuously saturated the piezo transducer with low-frequency energy.
- The head stepping transients ($31.3\text{ ms}$ duration) had an excursion of only $\approx \pm 1,410$ counts ($\approx 25\%$ PWM swing).
- As a result, the stepping clicks drowned in the dominant motor hum.

## 2. Proposed Architecture & Gain Calibration

### 2.1 Spindle Motor Hum Gain (`pico1541-common::motor_sample::volume_gain`)
Reduce gain by approximately 4x so that the motor hum acts as a subtle background presence:
- **`SoundVolume::Off`**: `0`
- **`SoundVolume::Low`**: `(top as i32) / 1000` (at `TOP = 11337`, gain is `11`; peak excursion $\approx \pm 275$ counts)
- **`SoundVolume::Medium`**: `(top as i32) / 400` (at `TOP = 11337`, gain is `28`; peak excursion $\approx \pm 700$ counts)
- **`SoundVolume::High`**: `(top as i32) / 200` (at `TOP = 11337`, gain is `56`; peak excursion $\approx \pm 1400$ counts)

### 2.2 Stepper Clatter Gain (`pico1541-common::stepper_sample::volume_gain`)
Boost gain by approximately 2.4x – 2.5x so that the stepping transients cut cleanly through the motor hum:
- **`SoundVolume::Off`**: `0`
- **`SoundVolume::Low`**: `(top as i32) / 120` (at `TOP = 11337`, gain is `94`; nominal transient excursion $\approx \pm 1,410$ counts)
- **`SoundVolume::Medium`**: `(top as i32) / 50` (at `TOP = 11337`, gain is `226`; nominal transient excursion $\approx \pm 3,390$ counts)
- **`SoundVolume::High`**: `(top as i32) / 30` (at `TOP = 11337`, gain is `377`; nominal transient excursion $\approx \pm 5,655$ counts)

### 2.3 Headroom & Mixed Waveform Dynamic Range
In `mixed_sample_to_pwm_duty_faded`:
- Center DC bias is `TOP / 2 = 5,668` ($1.65\text{ V}$).
- At `Medium` volume:
  - Motor hum nominal excursion: $\approx \pm 700$ counts ($12\%$ of half-scale).
  - Stepper nominal excursion: $\approx \pm 3,390$ counts ($60\%$ of half-scale).
  - Combined nominal excursion: $700 + 3390 = 4090 < 5668$, which fits cleanly within the linear range without unneeded clipping.
  - Rare peak samples in `STEPPING_SAMPLE` (up to $+31$) combined with motor peaks gently saturate at the rails via `.clamp(0, top as i32)`, adding sharp mechanical impact.
  - `BUMP_SAMPLE` peaks (up to $\pm 101$) clip at the rails during head bumps, reproducing the violent mechanical impact of the head hitting the track 0 stop.

## 3. Scope of Modifications
1. `crates/pico1541-common/src/motor_sample.rs`:
   - Update `volume_gain` with new divisors (1000, 400, 200).
2. `crates/pico1541-common/src/stepper_sample.rs`:
   - Update `volume_gain` with new divisors (120, 50, 30).
3. `crates/pico1541-common/tests/motor_sample_tests.rs`:
   - Update unit test assertions for gain thresholds and excursion magnitudes.
4. `crates/pico1541-common/tests/stepper_sample_tests.rs`:
   - Update unit test assertions for gain thresholds and excursion magnitudes.
5. All firmware crates remain `#![no_std]`.
6. Zero changes needed to DMA hardware logic in `pico1541-rp-common` since the in-place circular buffer mixing architecture already consumes `volume_gain` and `mixed_sample_to_pwm_duty_faded`.

## 4. Testing & Verification
- Run `cargo test` to verify all unit tests pass with the new gains.
- Verify `cargo check -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release` compiles with zero warnings.
