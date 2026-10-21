# 1541 Authentic Stepper & Bump Sample Playback Design Specification

## Overview & Goals
Driving a capacitive piezo transducer with artificial single-frequency square-wave tones (whether 600 Hz or 1,200 Hz) causes two major acoustic problems:
1. Low frequencies (like 600 Hz) drop far below the piezo's natural mechanical resonance frequency (~3 kHz – 4.5 kHz), resulting in near-silence.
2. The brain perceives any continuous periodic tone as an electronic "beep" or "chirp", lacking the physical damping, impulse transients, and multi-frequency resonance of a real mechanical drive.

This specification replaces synthetic square-wave stepper tones with **authentic 1541 mechanical recordings** extracted from VICE:
- **`STEPPING_SAMPLE` (691 samples @ 22,050 Hz, $\approx 31.3\text{ ms}$):** The real mechanical "snap/clack" of the 1541 stepper motor rotating and pulling the head carriage into the next track detent.
- **`BUMP_SAMPLE` (2,616 samples @ 22,050 Hz, $\approx 118.6\text{ ms}$):** The authentic mechanical knock when the carriage slams into the track 1 mechanical stop during drive recalibration.

Both samples are played through the existing **22,050 Hz ultrasonic hardware PWM carrier** (GPIO 2, PWM Slice 1) using hardware DMA, ensuring **0% Core 0 CPU utilization**, cycle-exact pacing, and authentic mechanical acoustics.

---

## Architecture & Hardware Pacing

### 1. Dual-Core & Hardware Isolation
- **Core 0 (Manager & Audio):** Owns `sound_task` and `PwmPiezo`. Streams sample audio via DMA Channels 2 & 3.
- **Core 1 (Emulator):** Completely isolated; continues cycle-exact 1 MHz execution. Emits `SOUND_STEP_COUNTER` and `SOUND_BUMPED_STOP` lock-free atomics without cycle penalty.
- **Hardware Resources:**
  - GPIO 2: PWM Slice 1 Channel A (carrier frequency 22,050 Hz, `TOP = 11,337`).
  - DMA Channel 2: Audio Worker Channel (paced by `DREQ_PWM_WRAP1`).
  - DMA Channel 3: Audio Reloader Channel (ping-pong reset for infinite loops).

---

## Detailed Design

### 1. Sample Data & Duty Scaling (`pico1541-common`)
A new module `crates/pico1541-common/src/stepper_sample.rs` will contain:
- `pub const STEPPING_SAMPLE_RATE: u32 = 22050;`
- `pub const STEPPING_SAMPLE: [i8; 691];`
- `pub const BUMP_SAMPLE: [i8; 2616];`
- Duty cycle converter function:
  ```rust
  #[inline]
  pub fn stepper_sample_to_pwm_duty(sample: i8, top: u16, volume: SoundVolume) -> u16 {
      if volume == SoundVolume::Off {
          return 0;
      }
      let center = (top / 2) as i32;
      let gain = match volume {
          SoundVolume::Off => 0,
          SoundVolume::Low => (top as i32) / 250,
          SoundVolume::Medium => (top as i32) / 130,
          SoundVolume::High => (top as i32) / 80,
      };
      let duty = center + (sample as i32 * gain);
      duty.clamp(0, top as i32) as u16
  }
  ```

### 2. Stepper Playback in `PwmPiezo` (`pico1541-rp-common`)

#### A. Active Stepper Clicks (Motor Running / Fading)
When the spindle motor is humming, the 22,050 Hz carrier is active with a constant 1.65V DC center.
1. `PwmPiezo::play_active_step_click(&mut self, volume: SoundVolume, is_bump: bool)`:
   - Populates the active step buffer (691 halfwords for step, or 2,616 for bump) scaled by `stepper_sample_to_pwm_duty`.
   - Pauses Worker DMA Channel 2.
   - Redirects Worker Channel 2 to the stepper duty buffer (`read_addr = STEPPER_DUTY_BUFFER`, `trans_count = 691` or `2616`).
   - Configures Channel 2 chaining to Reloader Channel 3 (`ctrl.set_chain_to(DMA_CH_RELOADER)`).
   - Triggers Channel 2.
   - **Chaining Sequence:**
     - Channel 2 streams the 31.3 ms mechanical step sample directly to the PWM compare register at 22,050 Hz.
     - As soon as the last sample completes, Channel 2 chains to Reloader Channel 3.
     - Reloader Channel 3 automatically resets Channel 2 to `MOTOR_DUTY_BUFFER` and restarts the infinite motor hum loop!
   - **Acoustic Outcome:**
     - `[Motor Hum] -> [Authentic 31ms Mechanical Snap] -> [Motor Hum resumes]`
     - Zero DC bias drops, zero pops, zero beeping.
     - 100% hardware DMA streaming with 0% CPU overhead.

#### B. Stopped Stepper Clicks (Motor Stopped)
When the spindle motor is off:
1. `PwmPiezo::play_stopped_step_click(&mut self, volume: SoundVolume, is_bump: bool)`:
   - Configures PWM Slice 1 with carrier `TOP = 11,337`.
   - Streams the 691-sample step (or 2,616-sample bump) via one-shot DMA Channel 2 (no chaining to Channel 3).
   - Once the transfer completes, clears PWM to 0V.
   - Sounds like a crisp, authentic mechanical click of the head stepping while the spindle is parked.

### 3. Integration in `sound_task`
In `crates/pico1541-rp-common/src/tasks/sound.rs`:
- When `steps > 0 && settings.stepper_enabled`:
  - If `envelope == MotorEnvelope::Stopped`:
    - Calls `piezo.play_stopped_step_click(settings.volume, is_bump).await`.
  - If `envelope` is active (`FadingIn`, `Running`, `FadingOut`):
    - Calls `piezo.play_active_step_click(settings.volume, is_bump).await`.
  - For multi-step seeks (`steps > 1`), enforces a brief inter-step gap (e.g. 5–10 ms) matching real 1541 head stepping speed.

---

## Resource Utilization & Footprint
- **Flash:** $\approx 3.3\text{ KB}$ for `STEPPING_SAMPLE` and `BUMP_SAMPLE` arrays.
- **SRAM:**
  - `STEPPER_DUTY_BUFFER`: $691 \times 2 = 1,382\text{ bytes}$.
  - Bump sound can stream either from a shared buffer or directly without extra allocation.
- **CPU:** **0% Core 0 CPU utilization** during playback (autonomous DMA).
- **Core 1:** 100% isolated, 0 cycle impact.
- **`#![no_std]`:** Strictly preserved across all firmware crates.
