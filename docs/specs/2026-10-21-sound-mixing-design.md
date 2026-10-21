# 1541 Continuous-Phase Audio Mixing Design Specification

## Overview & Problem Statement

Currently, when a head step or bump event occurs while the spindle motor is running, the sound driver pauses the circular motor hum DMA channel and redirects Worker Channel 2 to play only the stepper sample (`STEPPING_SAMPLE` or `BUMP_SAMPLE`). When the step completes (after 32 ms or 120 ms), the driver restarts the motor hum DMA from index 0.

This causes two audible acoustic defects:
1. **Motor Dropout:** During head movement (and especially multi-track seeks), the motor sound repeatedly cuts out and disappears.
2. **Phase Discontinuity / Stutter:** Resetting the motor buffer read pointer to 0 after every step causes abrupt phase jumps in the 300 RPM hum waveform, creating a garbled, stuttering sound.

In authentic 1541 hardware and emulator references like VICE (`drive-sound.c`), the motor sound never stops during head movement. Instead, the motor hum and the mechanical head stepping transient are **linearly mixed together**:
$$\text{output}(t) = \text{motor}(t) + \text{step}(t)$$

This specification introduces **Continuous-Phase Audio Mixing** for `PwmPiezo`.

---

## Architectural Design

### 1. Dual-Core Separation & Zero CPU Overhead
- **Core 1 (Emulator):** Completely decoupled. Dispatches step count and bump atomic flags lock-free.
- **Core 0 (Manager & Audio):** 
  - Streams audio via hardware DMA Channels 2 & 3 paced by `DREQ_PWM_WRAP1` at 22,050 Hz ultrasonic carrier (`TOP = 11,337`).
  - CPU usage during sample playback remains **0%** (DMA autonomous streaming). Mixing occurs once per step in the async task with negligible CPU time (~10 µs for 691 additions on a 150/250 MHz Cortex-M33).

---

## Detailed Components

### 1. Digital Mixing Math (`pico1541-common`)
In `crates/pico1541-common/src/stepper_sample.rs`:
Add duty calculation functions that combine the signed 8-bit motor hum and stepper samples:

```rust
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
    let full_step_gain = match volume {
        SoundVolume::Off => 0,
        SoundVolume::Low => (top as i32) / 250,
        SoundVolume::Medium => (top as i32) / 120,
        SoundVolume::High => (top as i32) / 70,
    };

    let motor_gain = (full_motor_gain * clamped_step as i32) / (max_steps_u32 as i32);
    let step_gain = (full_step_gain * clamped_step as i32) / (max_steps_u32 as i32);

    let ac = (motor_sample as i32 * motor_gain) + (step_sample as i32 * step_gain);
    let duty = center + ac;
    duty.clamp(0, top as i32) as u16
}
```

When `step == max_steps` (steady-state `Running`), `clamped_step / max_steps == 1`, center is exactly `top / 2` (1.65V DC), and both AC gains operate at full volume.

---

### 2. Real-Time Phase Synchronization & DMA Handoff (`pico1541-rp-common`)

#### Buffer Population with Phase Alignment
When `play_active_step_click` is invoked:
1. Sample the current read pointer from Worker DMA Channel 2:
   ```rust
   let curr_addr = pac::DMA.ch(DMA_CH_WORKER).read_addr().read() as usize;
   let base_addr = core::ptr::addr_of!(MOTOR_DUTY_BUFFER) as usize;
   let offset_bytes = curr_addr.saturating_sub(base_addr);
   let curr_idx = (offset_bytes / 2) % MOTOR_SAMPLE_COUNT;
   ```
2. Populate `STEPPER_DUTY_BUFFER[0..len]`:
   For each sample $i \in 0..\text{len}$:
   - `motor_idx = (curr_idx + i) % MOTOR_SAMPLE_COUNT`
   - Calculate `mixed_sample_to_pwm_duty_faded(MOTOR_HUM_SAMPLE[motor_idx], STEP_SAMPLE[i], ...)`
   - Write to `STEPPER_DUTY_BUFFER[i]`.
3. Memory barrier (`dmb` + `Release` compiler fence).

#### Seamless DMA Handoff & Seamless Return
1. Pause Worker DMA Channel 2 (`Self::pause_motor_dma()`).
2. Point Worker Channel 2 to `STEPPER_DUTY_BUFFER`:
   - `read_addr = addr_of!(STEPPER_DUTY_BUFFER)`
   - `trans_count = len` (691 for step, 2616 for bump)
   - `chain_to = DMA_CH_WORKER` (unchained)
   - Enable & trigger.
3. Asynchronously await sample completion:
   - 32 ms for regular step (691 samples @ 22,050 Hz = 31.3 ms).
   - 120 ms for track 1 bump (2616 samples @ 22,050 Hz = 118.6 ms).
4. Resume `MOTOR_DUTY_BUFFER` at the exact forward phase position:
   - `resume_idx = (curr_idx + len) % MOTOR_SAMPLE_COUNT`
   - `remaining_samples = MOTOR_SAMPLE_COUNT - resume_idx`
   - Pause Worker Channel 2.
   - Point `read_addr` to `base_addr + resume_idx * 2`.
   - Set `trans_count` to `remaining_samples`.
   - Configure `chain_to = DMA_CH_RELOADER`.
   - Enable & trigger.
5. When `remaining_samples` finish playing, Channel 2 chains to Reloader Channel 3, which reloads `base_addr` and the full `MOTOR_SAMPLE_COUNT` transfer count, returning to circular infinite looping with zero phase error.

---

## Acoustic & Physical Outcomes

- **Zero Motor Dropout:** The motor sound never stops or dips in volume when seeking or bumping.
- **Zero Phase Discontinuity:** The motor hum maintains continuous phase through every step.
- **Natural Mechanical Layering:** The head click is heard as a crisp physical transient riding naturally on top of the hum, perfectly matching real 1541 acoustic behavior.
- **Zero DC Offset Pops:** All mixing is DC-referenced to the ongoing envelope center ($1.65\text{ V}$).
