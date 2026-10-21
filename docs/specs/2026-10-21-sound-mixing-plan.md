# Continuous-Phase Audio Mixing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement digital audio mixing of the spindle motor hum and mechanical head stepping/bump samples with continuous phase tracking to eliminate motor dropouts and stutter during seeking.

**Architecture:**
In `pico1541-common/src/stepper_sample.rs`, `mixed_sample_to_pwm_duty_faded` calculates the combined AC excursions of the spindle motor hum and mechanical head click centered around the ongoing DC bias ($1.65\text{ V}$ at steady-state).
In `pico1541-rp-common/src/driver/speaker.rs`, `play_active_step_click` samples Worker DMA Channel 2's current read address to find the exact phase index $curr$ within the circular motor hum buffer. It populates `STEPPER_DUTY_BUFFER` with the mixed waveform starting at $curr$, streams it unchained over the ultrasonic PWM carrier, and upon completion smoothly resumes `MOTOR_DUTY_BUFFER` at $(curr + len) \pmod{8819}$.
In `pico1541-rp-common/src/tasks/sound.rs`, `sound_task` passes the current envelope progress step to `play_active_step_click`.

**Architecture Diagram:**

```mermaid
graph TD
    subgraph "Core 1 (Emulator)"
        EMU[6502 / VIA Stepper Event] --> EVT[SOUND_STEP_COUNTER Atomic]
    end

    subgraph "Core 0 (Manager & Audio)"
        EVT -->|get_and_clear_steps| ST[sound_task]
        ST -->|Envelope Active| PAS[PwmPiezo::play_active_step_click]
        
        PAS -->|Read current DMA ptr| RD[DMA_CH_WORKER.read_addr]
        RD -->|Calculate phase index| IDX[curr_idx in MOTOR_DUTY_BUFFER]
        
        IDX -->|Mix motor[curr_idx + i] + step[i]| MIX[mixed_sample_to_pwm_duty_faded]
        MIX --> SDB[STEPPER_DUTY_BUFFER]
        
        SDB -->|Stream mixed audio via DMA| DMA2[DMA Channel 2 Worker]
        DMA2 -->|Paced by DREQ_PWM_WRAP1| PWM[PWM Slice 1 Channel A GPIO 2]
        
        PAS -.->|After 32ms/120ms| RESUME[Resume MOTOR_DUTY_BUFFER at curr_idx + len]
        RESUME --> DMA2
    end

    subgraph "pico1541-common"
        MHS[MOTOR_HUM_SAMPLE] --> MIX
        SS[STEPPING_SAMPLE / BUMP_SAMPLE] --> MIX
    end
```

**Tech Stack:** Rust (`#![no_std]`), RP2350 (Cortex-M33), Embassy RP, Hardware DMA (Channels 2 & 3), PWM Slice 1 Channel A (GPIO 2).

## Global Constraints

- `#![no_std]` compliance across all firmware crates (`pico1541-common`, `pico1541-rp-common`, `pico1541-rp2350`).
- No dynamic memory allocation (`no_std`).
- Dual-core separation: Core 0 owns audio DMA and manager tasks; Core 1 emulation execution remains strictly cycle-exact and 100% isolated.
- 0% Core 0 CPU utilization during sample streaming (hardware DMA paced by `DREQ_PWM_WRAP1`).
- Ultrasonic carrier frequency strictly maintained at 22,050 Hz (`TOP = 11,337`).
- Zero DC switching clicks: continuous $1.65\text{ V}$ DC center during active motor playback.
- Target compilation: `thumbv8m.main-none-eabihf` in `--release` mode with zero compiler warnings.
- Git workflow: stay on branch `feature/sound-mixing`. Do **NOT** merge into `main` or create a PR.

---

### Task 1: Mixed Duty Math in `pico1541-common`

**Files:**
- Modify: `crates/pico1541-common/src/stepper_sample.rs`
- Modify: `crates/pico1541-common/tests/stepper_sample_tests.rs`

**Interfaces:**
- Produces:
  - `pub fn mixed_sample_to_pwm_duty_faded(motor_sample: i8, step_sample: i8, top: u16, volume: SoundVolume, step: u8, max_steps: u8) -> u16`

- [ ] **Step 1: Write the failing unit tests**

In `crates/pico1541-common/tests/stepper_sample_tests.rs`:
Add:
```rust
use pico1541_common::stepper_sample::mixed_sample_to_pwm_duty_faded;

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
    let duty_both_pos = mixed_sample_to_pwm_duty_faded(20, 20, top, SoundVolume::High, 15, 15);
    let duty_motor_only = mixed_sample_to_pwm_duty_faded(20, 0, top, SoundVolume::High, 15, 15);
    let duty_step_only = mixed_sample_to_pwm_duty_faded(0, 20, top, SoundVolume::High, 15, 15);
    assert!(duty_both_pos > duty_motor_only);
    assert!(duty_both_pos > duty_step_only);
    assert!(duty_both_pos <= top);

    // Clamping limits
    let duty_max_clamp = mixed_sample_to_pwm_duty_faded(127, 127, top, SoundVolume::High, 15, 15);
    assert_eq!(duty_max_clamp, top);
    let duty_min_clamp = mixed_sample_to_pwm_duty_faded(-128, -128, top, SoundVolume::High, 15, 15);
    assert_eq!(duty_min_clamp, 0);

    // Envelope scaling at mid-step (e.g. 7 of 15)
    let duty_mid_dc = mixed_sample_to_pwm_duty_faded(0, 0, top, SoundVolume::High, 7, 15);
    let expected_center = ((top as u32 / 2) * 7 / 15) as u16;
    assert_eq!(duty_mid_dc, expected_center);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pico1541-common --test stepper_sample_tests`
Expected: FAIL (cannot find function `mixed_sample_to_pwm_duty_faded`)

- [ ] **Step 3: Implement `mixed_sample_to_pwm_duty_faded` in `crates/pico1541-common/src/stepper_sample.rs`**

```rust
/// Combines a signed 8-bit motor hum sample and a signed 8-bit stepper/bump sample
/// into a single 16-bit PWM counter duty value, scaled according to the envelope progress.
#[inline]
pub fn mixed_sample_to_pwm_duty_faded(
    motor_sample: i8,
    step_sample: i8,
    top: u16,
    volume: SoundVolume,
    step: u8,
    max_steps: u8,
) -> u16 {
    if volume == SoundVolume::Off || step == 0 || max_steps == 0 {
        return 0;
    }
    let clamped_step = step.min(max_steps) as u32;
    let max_steps_u32 = max_steps as u32;

    let full_center = (top / 2) as u32;
    let center = ((full_center * clamped_step) / max_steps_u32) as i32;

    let full_motor_gain = match volume {
        SoundVolume::Off => 0,
        SoundVolume::Low => (top as i32) / 250,
        SoundVolume::Medium => (top as i32) / 100,
        SoundVolume::High => (top as i32) / 60,
    };
    let full_step_gain = volume_gain(volume, top);

    let motor_gain = (full_motor_gain * clamped_step as i32) / (max_steps_u32 as i32);
    let step_gain = (full_step_gain * clamped_step as i32) / (max_steps_u32 as i32);

    let ac = (motor_sample as i32 * motor_gain) + (step_sample as i32 * step_gain);
    let duty = center + ac;
    duty.clamp(0, top as i32) as u16
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pico1541-common --test stepper_sample_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/pico1541-common/src/stepper_sample.rs crates/pico1541-common/tests/stepper_sample_tests.rs
git commit -m "feat(common): add mixed_sample_to_pwm_duty_faded for continuous motor and stepper mixing"
```

---

### Task 2: Phase-Synchronized Mixed DMA Streaming in `PwmPiezo` (`pico1541-rp-common`)

**Files:**
- Modify: `crates/pico1541-rp-common/src/driver/speaker.rs`

**Interfaces:**
- Consumes:
  - `pico1541_common::stepper_sample::mixed_sample_to_pwm_duty_faded`
  - `pico1541_common::motor_sample::MOTOR_HUM_SAMPLE`
- Produces:
  - `PwmPiezo::play_active_step_click(&mut self, volume: SoundVolume, is_bump: bool, envelope_step: u8, max_steps: u8)`

- [ ] **Step 1: Update `populate_stepper_buffer` in `speaker.rs` to support mixed audio**

```rust
fn populate_stepper_buffer_mixed(
    volume: SoundVolume,
    is_bump: bool,
    start_motor_idx: usize,
    envelope_step: u8,
    max_steps: u8,
) -> usize {
    let buf_ptr = core::ptr::addr_of_mut!(STEPPER_DUTY_BUFFER) as *mut u16;
    let len = if is_bump {
        pico1541_common::stepper_sample::BUMP_SAMPLE.len()
    } else {
        pico1541_common::stepper_sample::STEPPING_SAMPLE.len()
    };

    unsafe {
        for i in 0..len {
            let motor_idx = (start_motor_idx + i) % MOTOR_SAMPLE_COUNT;
            let motor_sample = MOTOR_HUM_SAMPLE[motor_idx];
            let step_sample = if is_bump {
                pico1541_common::stepper_sample::BUMP_SAMPLE[i]
            } else {
                pico1541_common::stepper_sample::STEPPING_SAMPLE[i]
            };
            *buf_ptr.add(i) = pico1541_common::stepper_sample::mixed_sample_to_pwm_duty_faded(
                motor_sample,
                step_sample,
                MOTOR_PWM_TOP,
                volume,
                envelope_step,
                max_steps,
            );
        }
    }
    cortex_m::asm::dmb();
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    len
}
```

- [ ] **Step 2: Update `play_active_step_click` with phase-synchronized handoff and resume**

```rust
pub async fn play_active_step_click(
    &mut self,
    volume: SoundVolume,
    is_bump: bool,
    envelope_step: u8,
    max_steps: u8,
) {
    if !self.motor_humming || volume == SoundVolume::Off {
        return;
    }

    let dma = pac::DMA;
    let ch_worker = dma.ch(DMA_CH_WORKER);

    // 1. Sample current read pointer to determine the exact motor hum phase index
    let curr_addr = ch_worker.read_addr().read() as usize;
    let base_addr = core::ptr::addr_of!(MOTOR_DUTY_BUFFER) as usize;
    let offset_bytes = curr_addr.saturating_sub(base_addr);
    let curr_idx = (offset_bytes / 2) % MOTOR_SAMPLE_COUNT;

    // 2. Populate STEPPER_DUTY_BUFFER with the mixed audio (motor hum + step clack)
    let len = Self::populate_stepper_buffer_mixed(volume, is_bump, curr_idx, envelope_step, max_steps);
    Self::pause_motor_dma();

    // 3. Configure Worker Channel 2 to stream the mixed buffer (unchained)
    ch_worker
        .read_addr()
        .write_value(core::ptr::addr_of!(STEPPER_DUTY_BUFFER) as u32);
    ch_worker
        .trans_count()
        .write_value(pac::dma::regs::ChTransCount(len as u32));

    cortex_m::asm::dmb();
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    ch_worker.ctrl_trig().modify(|w| {
        w.set_chain_to(DMA_CH_WORKER as u8);
        w.set_en(true);
    });

    // 4. Await sample playback completion (32 ms for step, 120 ms for bump)
    let duration_ms = if is_bump { 120 } else { 32 };
    embassy_time::Timer::after(embassy_time::Duration::from_millis(duration_ms)).await;

    // 5. Seamlessly resume circular motor hum at the exact forward phase position
    let resume_idx = (curr_idx + len) % MOTOR_SAMPLE_COUNT;
    let remaining_samples = MOTOR_SAMPLE_COUNT - resume_idx;
    let resume_addr = base_addr + (resume_idx * 2);

    Self::pause_motor_dma();
    ch_worker.read_addr().write_value(resume_addr as u32);
    ch_worker
        .trans_count()
        .write_value(pac::dma::regs::ChTransCount(remaining_samples as u32));

    cortex_m::asm::dmb();
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    ch_worker.ctrl_trig().modify(|w| {
        w.set_chain_to(DMA_CH_RELOADER as u8);
        w.set_en(true);
    });
}
```

- [ ] **Step 3: Verify target compilation**

Run: `cargo check -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Run: `cargo test`

- [ ] **Step 4: Commit**

```bash
git add crates/pico1541-rp-common/src/driver/speaker.rs
git commit -m "feat(rp-common): implement continuous-phase mixed DMA streaming in PwmPiezo"
```

---

### Task 3: Task Integration & Envelope Step Propagation (`pico1541-rp-common`)

**Files:**
- Modify: `crates/pico1541-rp-common/src/tasks/sound.rs`

**Interfaces:**
- Consumes:
  - `PwmPiezo::play_active_step_click` with `envelope_step` parameter

- [ ] **Step 1: Pass envelope step in `sound_task`**

In `crates/pico1541-rp-common/src/tasks/sound.rs`:
Extract current envelope progress step helper:
```rust
let (current_step, max_fade_steps) = match envelope {
    MotorEnvelope::Stopped => (0, FADE_STEPS),
    MotorEnvelope::FadingIn { step } => (step, FADE_STEPS),
    MotorEnvelope::Running => (FADE_STEPS, FADE_STEPS),
    MotorEnvelope::FadingOut { step } => (step, FADE_STEPS),
};
```
In Section 2 (Stepper Click Handling):
```rust
                if envelope == MotorEnvelope::Stopped {
                    // Motor stopped: authentic mechanical sample with smooth DC ramp
                    piezo.play_stopped_step_click(settings.volume, is_bump).await;
                } else {
                    // Motor active: authentic mechanical sample mixed with motor hum
                    piezo.play_active_step_click(settings.volume, is_bump, current_step, max_fade_steps).await;
                }
```

- [ ] **Step 2: Verify target compilation & unit tests**

Run: `cargo check -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Run: `cargo test`

- [ ] **Step 3: Commit**

```bash
git add crates/pico1541-rp-common/src/tasks/sound.rs
git commit -m "feat(sound): propagate envelope step to play_active_step_click in sound_task"
```

---

### Task 4: Target Verification & Final Review

**Files:**
- Verify: all crates

- [ ] **Step 1: Check compiler warnings on target**

Run: `cargo check -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Ensure 0 warnings.

- [ ] **Step 2: Run all unit tests**

Run: `cargo test`
Ensure 100% tests pass.

- [ ] **Step 3: Final Feature Code Review**

Invoke `code-reviewer` agent to perform whole-feature review against requirements and constraints.

- [ ] **Step 4: Prepare branch handoff for user**

Do **NOT** merge into `main` or create a PR. Hand off `feature/sound-mixing` to user.
