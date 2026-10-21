# 1541 Motor Sound Fade-In/Out & Click Elimination Design Specification

## Overview & Goals
When the spindle motor starts or stops on a physical piezo speaker, the sudden transition between 0V (PWM off) and 1.65V (50% duty ultrasonic carrier @ 22,050 Hz) creates a sharp capacitive step impulse across the piezo transducer, heard as an audible click or pop. In addition, an abrupt audio onset lacks the realistic acoustic inertia of a spinning drive spindle.

This specification introduces:
1. A smooth **150 ms fade-in and fade-out envelope** for the spindle motor sound that simultaneously ramps both the DC bias ($0\text{V} \leftrightarrow 1.65\text{V}$) and the PCM sample amplitude.
2. A **seamless stepper click overlay** that preserves the 22,050 Hz carrier frequency and DC bias during head stepping, allowing the motor hum to sound continuous underneath stepper clicks without clicks, pops, or dropouts.

---

## Architecture & Hardware Pacing

### 1. Dual-Core & Hardware Isolation
- **Core 0 (Manager & Audio):** Owns `sound_task` and `PwmPiezo`. Envelope calculations run exclusively on Core 0 during the 150 ms fade transitions.
- **Core 1 (Emulator):** Maintains strict cycle-exact 1 MHz execution. Communicates purely via existing lock-free atomics (`SOUND_MOTOR_ON`, `SOUND_STEP_COUNT`).
- **Hardware Resources:**
  - GPIO 2: PWM Slice 1 Channel A.
  - DMA Channel 2: Motor Hum Worker (paced by `DREQ_PWM_WRAP1`).
  - DMA Channel 3: Motor Hum Reloader (ping-pong reset writing to Ch 2 `AL3_READ_ADDR_TRIG`).

---

## Detailed Design

### 1. Motor Hum Fade Envelope
- **Transition Duration:** 150 ms total for spin-up (fade-in) and spin-down (fade-out).
- **Update Rate:** 10 ms periodic interval (15 steps total).
- **Envelope Calculation:**
  For step $k \in [0, 15]$, progress $\alpha = k / 15.0 \in [0.0, 1.0]$.
  - **DC Center Ramp:**
    $$\text{center}(\alpha) = \left\lfloor \frac{\text{TOP}}{2} \times \alpha \right\rfloor$$
    - At $\alpha = 0.0$ (rest): $\text{center} = 0$, PWM output is held at 0V.
    - Slew rate is $1.65\text{V} / 150\text{ms} = 11\text{ V/s}$ ($< 5\text{ Hz}$ bandwidth), rendering the DC transition completely inaudible to human ears and incapable of exciting the piezo transducer resonance.
  - **AC Amplitude Ramp:**
    $$\text{gain}(\text{volume}, \alpha) = \text{nominal\_gain}(\text{volume}) \times \alpha$$
    $$\text{duty}(sample, \alpha) = \text{clamp}\left(\text{center}(\alpha) + (sample \times \text{gain}(\text{volume}, \alpha)), 0, \text{TOP}\right)$$

### 2. Envelope State Machine in `sound_task`
The motor playback lifecycle is modeled as an envelope state machine:
```rust
enum MotorEnvelope {
    Stopped,
    FadingIn { step: u8, target_volume: SoundVolume },
    Running,
    FadingOut { step: u8 },
}
```
- **From `Stopped` to `FadingIn`:** Triggered when `is_motor_running() && settings.motor_enabled`. Starts DMA with zeroed buffer at $\alpha = 0.0$, then advances $\alpha$ every 10 ms.
- **From `FadingIn` to `Running`:** Reached when step 15 ($\alpha = 1.0$) completes. Steady state: DMA runs autonomously with **0% Core 0 CPU load**.
- **From `Running` to `FadingOut`:** Triggered when `!is_motor_running() || !settings.motor_enabled` or `volume == Off`. Decrements $\alpha$ every 10 ms.
- **From `FadingOut` to `Stopped`:** Reached when $\alpha = 0.0$. At this point, the buffer is zero, DMA channels are halted, and PWM is cleared to 0V with zero step discontinuity.

### 3. Seamless Stepper Click Overlay
When head stepping occurs (`steps > 0 && settings.stepper_enabled`):

#### A. When Motor is `Stopped`
1. Reconfigure PWM slice for 1.2 kHz tone (`TOP = 3255`, 50% duty).
2. Play tone for 1.8 ms (2.8 ms for track 1 head bump knock).
3. Return PWM to 0V.

#### B. When Motor is Active (`Running`, `FadingIn`, or `FadingOut`)
1. Keep PWM carrier locked at 22,050 Hz (`TOP = 11,337`).
2. Temporarily pause Worker DMA Channel 2 by clearing enable (`ch_worker.al1_ctrl().modify(|w| *w &= !DMA_CHANNEL_ENABLE_MASK)`).
3. While paused, drive the PWM compare register around the existing DC center ($\text{TOP} / 2 = 5668$):
   - Modulate $\pm \text{click\_amp}$ at 1.2 kHz (416 µs high at $\text{center} + \text{amp}$, 416 µs low at $\text{center} - \text{amp}$) for 2 cycles ($\approx 1.66\text{ ms}$).
4. Immediately re-enable Worker Channel 2 (`ch_worker.ctrl_trig().modify(|w| w.set_en(true))`).
5. **Acoustic Outcome:**
   - DC bias remains constant at 1.65V throughout the entire step—zero DC transient pop.
   - 1.66 ms pause is psychoacoustically masked by the loud 1.2 kHz mechanical click.
   - Motor hum resumes immediately without re-triggering ping-pong DMA or recalculating duty buffers.

---

## Performance & Resource Utilization
- **Core 0 CPU Cost:**
  - Steady State: **0% CPU** (autonomous circular DMA).
  - Transition (150 ms fade): $\approx 140\text{ }\mu\text{s}$ per 10 ms step ($< 1.5\%$ Core 0 CPU load).
- **Core 1 CPU Cost:** **0% CPU** (completely isolated).
- **SRAM Usage:** Reuses existing `MOTOR_DUTY_BUFFER: [u16; 8819]` in place; **0 bytes** additional allocation.
- **`#![no_std]`:** Strictly preserved across all firmware crates.
