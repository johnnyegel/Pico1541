# 1541 Motor Sound Sample Playback — Design Specification

## Overview
The mechanical sound emulation on Pico1541 previously synthesized spindle motor hum as a 110 Hz square wave. Due to the high resonant frequency of the piezo transducer (~2 kHz - 4 kHz) and the rich odd harmonics of a square wave, this sounded harsh and buzzer-like, even at low volume.

This specification details the replacement of the synthesized 110 Hz square wave with authentic motor sound sample playback using the 1541 spindle motor recording from VICE, streamed autonomously via hardware PWM DMA on the RP2350.

---

## 1. Audio Sample & Data Format

### 1.1 Authentic VICE 1541 Recording
- **Source:** Pre-recorded 1541 drive motor hum sample (`hum[]` from VICE `src/drive/drive-sound.c`).
- **Sample Count:** 8,819 samples (signed 8-bit `i8`, amplitude range approximately -25 to +25, centered around 0).
- **Sampling Rate:** 22,050 Hz (exact duration 0.3999s, seamless loop).
- **Storage:** Stored in flash as `&'static [i8]` in `pico1541-common` or `pico1541-rp-common`.

---

## 2. Hardware Architecture & PWM DMA Engine

### 2.1 Ultrasonic PWM Carrier
- **Pin / Slice:** GPIO 2, PWM Slice 1 Channel A.
- **Clock:** System clock 250 MHz (`SYSTEM_CLOCK_FREQ`).
- **PWM Pacing Rate:** 22,050 Hz wrap rate (`TOP = 250_000_000 / 22,050 = 11,337` ticks) or 44,100 Hz (`TOP = 5,668` ticks) with doubled samples.
  - At 44.1 kHz, the carrier frequency is completely ultrasonic and inaudible to human ears.
  - The external RC / damping circuit and piezo capacitance naturally low-pass filter the PWM pulses, reproducing only the acoustic mechanical spectrum of the spinning spindle motor (bearing whir and low friction noise).

### 2.2 Autonomous DMA Streaming (Zero CPU Load)
- **DMA Channel Setup:**
  - Paced by PWM Slice 1 Wrap DREQ (`DREQ_PWM_WRAP1`).
  - Writes 16-bit compare values directly to PWM Slice 1 Counter-Compare (`ch1_cc` / `compare_a`).
  - Autonomous circular looping using two-channel ping-pong or chained DMA reload from DBG_TCR.
  - Runs in hardware with **0% Core 0 CPU utilization**, preventing any interference with the OLED display, menu navigation, or USB/IEC operations.

### 2.3 Volume & Mute Handling
- **Sample Translation:**
  $$\text{compare\_a} = \frac{\text{TOP}}{2} + (\text{sample} \times \text{volume\_gain})$$
  - Center point is $\text{TOP}/2$ (50% duty cycle).
  - `Off`: Volume gain = 0, PWM stopped (`TOP = 0, compare = 0`), output held LOW. Driver transistor remains off, drawing 0 mA quiescent current.
  - `Low`: Gentle volume gain (subtle whir).
  - `Medium`: Balanced acoustic level.
  - `High`: Clear mechanical feedback.
- Changing volume in the OLED menu dynamically scales the amplitude around the midpoint without requiring drive reset or re-allocations.

---

## 3. Interaction with Stepper & Head Bumps

- When stepper phase changes or track 1 head bumps occur:
  - Stepper clicks (1.2 kHz, 1.8 ms duration) and head knocks (2.8 ms duration) are played with high priority.
  - Motor DMA playback is briefly paused or overridden during the click duration (1.8 ms / 2.8 ms), then resumes seamlessly.
  - If motor is stopped (`SOUND_MOTOR_ON == false`), stepper clicks play without any trailing motor sound.

---

## 4. Components & File Locations

1. **`crates/pico1541-common/src/sound.rs`**:
   - Provide the embedded `MOTOR_HUM_SAMPLE: [i8; 8819]` or access helper.
2. **`crates/pico1541-rp-common/src/driver/speaker.rs`**:
   - Extend `PwmPiezo` to support DMA sample streaming mode on PWM Slice 1 Channel A.
3. **`crates/pico1541-rp-common/src/tasks/sound.rs`**:
   - Update `sound_task` to control motor sample DMA start/stop/volume update instead of square-wave tone generation.
4. **`crates/pico1541-rp2350/src/main.rs`**:
   - Pass DMA channel peripheral to sound driver initialization.

---

## 5. Verification Plan

1. **Host Unit Tests (`cargo test`)**:
   - Verify sample data integrity, length, and scaling formulas.
   - Verify `SoundVolume` duty and gain calculation.
2. **Hardware Target Compilation**:
   - `cargo build -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release` compiles with 0 errors and 0 warnings.
3. **Auditory & Timing Verification**:
   - Spindle motor hum is soft and realistic, free of 110 Hz square-wave buzz.
   - Volume settings (`Low`, `Medium`, `High`) scale acoustic level cleanly.
   - Stepper clicks and head bumps remain sharp and distinct.
