# 1541 Motor Sound Sample Playback — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the harsh 110 Hz square-wave spindle motor buzzer with authentic 1541 motor audio sample playback using hardware PWM DMA on RP2350, eliminating 110 Hz square-wave harmonics and providing realistic mechanical sound with zero Core 0 CPU overhead.

**Architecture:**
- **Sample Source:** Pre-recorded authentic 1541 motor hum sample (8,819 samples at 22,050 Hz) from VICE embedded in flash.
- **Hardware PWM Carrier:** Ultrasonic carrier (~44.1 kHz / 22.05 kHz) on PWM Slice 1 Channel A (GPIO 2) with hardware wrap DREQ.
- **DMA Streaming:** Autonomous circular DMA transfers compare values directly into the PWM compare register with 0% Core 0 CPU utilization.
- **Stepper Click Priority:** Stepper clicks and track 1 bump knocks smoothly override motor sound during steps, then hum resumes.

**Architecture Diagram:**

```mermaid
graph TD
    subgraph "Core 1 - Emulator"
        VIA2[VIA2 Motor & Stepper Writes] --> SoundAtomics[Sound State Atomics: Step / Motor]
    end

    subgraph "Core 0 - Manager"
        SoundAtomics --> SoundTask[Embassy SoundTask]
        SampleTable[Flash: 1541 Motor Hum Sample (8.8k i8)] --> AudioBuffer[PWM Duty Buffer]
        SoundTask --> |Start / Stop / Scale| PwmPiezo[Hardware PwmPiezo Driver]
        AudioBuffer --> |Circular Ping-Pong DMA| PwmSlice[PWM Slice 1 Channel A (GPIO 2)]
        SoundTask -.-> |Override during step click (1.2 kHz)| PwmSlice
    end

    subgraph "Hardware"
        PwmSlice --> Piezo[Transistor -> Piezo Speaker]
    end
```

**Tech Stack:** Rust (`no_std`), Embassy RP (`embassy-rp::pwm`, `embassy-rp::dma`), `pico1541-common`, `pico1541-rp-common`, `pico1541-rp2350`.

## Global Constraints
- Target architecture: `thumbv8m.main-none-eabihf` (RP2350 Cortex-M33).
- Must remain `#![no_std]` across all firmware crates.
- Lock-free cross-core atomics (`portable_atomic`) between Core 1 and Core 0.
- Zero CPU cycle delay on Core 1 emulation thread.
- Zero audio-induced CPU starvation on Core 0 Embassy executor.
- All workspace unit and integration tests must pass (`cargo test`).
- Target release build must compile with 0 warnings.

---

### Task 1: Authentic 1541 Motor Hum Sample Table & Duty Mapping (`pico1541-common`)

**Files:**
- Create: `crates/pico1541-common/src/motor_sample.rs`
- Modify: `crates/pico1541-common/src/sound.rs`
- Modify: `crates/pico1541-common/src/lib.rs`
- Test: `crates/pico1541-common/tests/motor_sample_tests.rs`

**Interfaces:**
- Produces:
  - `pub const MOTOR_HUM_SAMPLE: [i8; 8819]`
  - `pub const MOTOR_SAMPLE_RATE: u32 = 22050`
  - `pub fn sample_to_pwm_duty(sample: i8, top: u16, volume: SoundVolume) -> u16`

- [ ] **Step 1: Write unit test in `crates/pico1541-common/tests/motor_sample_tests.rs`**

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pico1541-common --test motor_sample_tests`
Expected: FAIL (module `motor_sample` does not exist).

- [ ] **Step 3: Implement `crates/pico1541-common/src/motor_sample.rs`**

Add `MOTOR_HUM_SAMPLE: [i8; 8819]` (the authentic 1541 recording from VICE) and `sample_to_pwm_duty`:

```rust
use crate::sound::SoundVolume;

pub const MOTOR_SAMPLE_RATE: u32 = 22050;

pub const MOTOR_HUM_SAMPLE: [i8; 8819] = [
    // 8819 samples from VICE drive-sound.c hum[] array
    // ...
];

#[inline]
pub fn sample_to_pwm_duty(sample: i8, top: u16, volume: SoundVolume) -> u16 {
    if volume == SoundVolume::Off {
        return 0;
    }

    let center = (top / 2) as i32;
    // Maximum raw sample amplitude in hum[] is ~25.
    // Scale factor per volume level (normalized to fit well within [0, top])
    let gain = match volume {
        SoundVolume::Off => 0,
        SoundVolume::Low => (top as i32) / 250,   // ~±10% peak modulation
        SoundVolume::Medium => (top as i32) / 100,// ~±25% peak modulation
        SoundVolume::High => (top as i32) / 60,   // ~±40% peak modulation
    };

    let duty = center + (sample as i32 * gain);
    duty.clamp(0, top as i32) as u16
}
```

- [ ] **Step 4: Export in `crates/pico1541-common/src/lib.rs` and verify tests pass**

Add `pub mod motor_sample;` in `crates/pico1541-common/src/lib.rs`.
Run: `cargo test -p pico1541-common --test motor_sample_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pico1541-common/
git commit -m "feat(common): add authentic 1541 motor hum sample and duty scaling"
```

---

### Task 2: Hardware PWM Sample Streaming & DMA Driver (`pico1541-rp-common`)

**Files:**
- Modify: `crates/pico1541-rp-common/src/driver/speaker.rs`
- Modify: `crates/pico1541-rp-common/src/tasks/sound.rs`

**Interfaces:**
- Consumes: `pico1541_common::motor_sample::*`, `embassy_rp::pwm::*`, `embassy_rp::dma::*`
- Produces: `PwmPiezo::start_motor_hum`, `PwmPiezo::stop_motor_hum`, `PwmPiezo::update_motor_volume`

- [ ] **Step 1: Extend `PwmPiezo` in `crates/pico1541-rp-common/src/driver/speaker.rs`**

Add support for:
1. Ultrasonic PWM carrier configuration at 22,050 Hz (`TOP = 11,337` ticks at 250 MHz).
2. Pre-computed or DMA-driven sample streaming into PWM Slice 1 compare register using `embassy_rp::dma::Channel`.
3. Safe switching between sample playback mode (motor hum) and tone mode (stepper clicks).

```rust
// PwmPiezo methods:
pub fn start_motor_hum(&mut self, volume: SoundVolume, dma: &mut impl embassy_rp::dma::Channel)
pub fn stop_motor_hum(&mut self)
pub fn update_motor_volume(&mut self, volume: SoundVolume)
```

- [ ] **Step 2: Add unit/integration verification**

Verify `crates/pico1541-rp-common` compiles cleanly for RP2350 target:
Run: `cargo check -p pico1541-rp-common --features rp235xa --target thumbv8m.main-none-eabihf`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/pico1541-rp-common/src/driver/speaker.rs
git commit -m "feat(rp-common): add DMA motor sample playback to PwmPiezo"
```

---

### Task 3: Integration in `sound_task` and Hardware Main (`pico1541-rp-common` & `pico1541-rp2350`)

**Files:**
- Modify: `crates/pico1541-rp-common/src/tasks/sound.rs`
- Modify: `crates/pico1541-rp2350/src/main.rs`

**Interfaces:**
- Consumes: `p.DMA_CH2`, `sound_task`, `PwmPiezo`
- Produces: Live motor sample playback in response to `SOUND_MOTOR_ON` atomics

- [ ] **Step 1: Update `sound_task` in `crates/pico1541-rp-common/src/tasks/sound.rs`**

In `sound_task`:
1. When `is_motor_running() && settings.motor_enabled`:
   - Start or continue motor hum sample playback.
2. When volume changes:
   - Call `piezo.update_motor_volume(settings.volume)`.
3. When `steps > 0 && settings.stepper_enabled`:
   - Temporarily pause motor hum, play 1.2 kHz click (2.8 ms knock on bump, 1.8 ms normal click), then resume motor hum if motor is still on.
4. When `!is_motor_running() || !settings.motor_enabled` or `volume == Off`:
   - Call `piezo.stop_motor_hum()`.

- [ ] **Step 2: Update `main.rs` in `crates/pico1541-rp2350`**

1. Pass DMA channel `p.DMA_CH2` into `Core0Pins` / sound driver setup.
2. Initialize `PwmPiezo` with PWM Slice 1, PIN 2, and DMA channel.

- [ ] **Step 3: Check compilation for RP2350 target**

Run: `cargo check -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Expected: PASS with 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/pico1541-rp-common/ crates/pico1541-rp2350/
git commit -m "feat(sound): integrate motor sample playback into sound_task and main"
```

---

### Task 4: Release Build, Full Workspace Test Suite, and Final Verification

**Files:**
- Test: Full cargo test suite & firmware binary compilation

- [ ] **Step 1: Run full unit test suite**

Run: `cargo test`
Expected: ALL PASS.

- [ ] **Step 2: Build release firmware binary**

Run: `cargo build -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Expected: Clean build, 0 warnings, binary generated.

- [ ] **Step 3: Commit final verification**

```bash
git commit --allow-empty -m "chore: complete 1541 motor sample playback implementation and verification"
```
