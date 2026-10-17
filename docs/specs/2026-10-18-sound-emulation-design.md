# 1541 Drive Sound Emulation Design Specification

## Overview
This document specifies the architecture and implementation for mechanical sound emulation on the Pico1541 drive using a passive Piezo speaker connected to GPIO 2 on the RP2350 microcontroller. The emulation reproduces:
1. Spindle motor spinning hum (~100–120 Hz pulse train).
2. Stepper motor head stepping chatter and seeks (~1.2 kHz acoustic clicks).
3. Head-knocking mechanical clatter when the carriage bumps against the track 1 stop.
4. User-configurable volume levels (Off, Low, Medium, High) and individual toggles for motor and stepper sounds via the OLED display and persisted in LittleFS flash.

## Hardware Specification
- **Pin:** GPIO 2 (RP2350 PWM Slice 1, Channel A).
- **Circuitry:** Active low-side transistor driver (Q1) switching +5V across the piezo element (LS1) with a 1kΩ parallel damping resistor (R1).
- **Transducer:** Passive crystal piezo buzzer. Frequency response has a resonant peak around 2–4 kHz and attenuates below 200 Hz. Narrow duty cycle pulse trains provide strong crystal displacement and realistic mechanical hum.

## Architectural Separation & Inter-Core Data Flow

### Core 1: Emulator Core (6502 + VIAs)
- Maintains strict 1 MHz cycle-exact emulation. No blocking, yielding, or audio timers on Core 1.
- In `crates/pico1541-emulator/src/drivemechanics.rs`:
  - When VIA2 Port B bits change:
    - Motor state (`VIA2_MASK_MOTOR` / PB2): Updates `SOUND_MOTOR_ON: AtomicBool`.
    - Stepper phase (`VIA2_MASK_STEP` / PB0–PB1):
      - Determines whether a phase shift occurred (`is_track_step`).
      - Detects if the head attempted to move beyond the physical track limit (`halftrack_head_pos <= MIN_HALF_TRACK`), indicating a physical stop collision (head bumping).
      - Increments `SOUND_STEP_COUNTER: AtomicU32` using `Ordering::Relaxed`.
      - If bumped against stop, sets `SOUND_BUMPED_STOP: AtomicBool` to `true`.
  - In `crates/pico1541-rp-common/src/emulation/bridge.rs`:
    - Bridges the `DriveMechanics` sound triggers into shared static atomic variables.

### Core 0: Manager Core (Embassy Async Runtime)
- Runs `SoundTask` which continuously polls/waits for sound state changes.
- Directly configures and modulates the RP2350 Hardware PWM on GPIO 2 (PWM Slice 1, Channel A).

```
 [ Core 1: 6502 Emulator ]
    │
    ├─ VIA2 Port B: PB2 Motor Toggle ────────► SOUND_MOTOR_ON: AtomicBool
    │
    └─ Stepper Phase (PB0/PB1) ──────────────► SOUND_STEP_COUNTER: AtomicU32
       (Including Track 1 Stop Bump)          SOUND_BUMPED_STOP: AtomicBool
                                                     │
                                                     ▼
                                          [ Core 0: SoundTask ]
                                                     │
               PWM Slice 1 Frequency & Duty ◄────────┤ Reads SoundSettings
                                                     │
                                                     ▼
                                            [ GPIO 2: Piezo ]
```

## Sound Synthesis Engine

### 1. Master Volume Control via PWM Duty Cycle
Passive piezo volume is modulated by varying pulse width (energy per cycle):
- **Off (0%):** PWM disabled, pin held low. Zero power dissipation.
- **Low (~5% duty cycle):** Low acoustic energy, quiet background sound.
- **Medium (~20% duty cycle):** Moderate volume, balanced for desktop use.
- **High (50% duty cycle):** Maximum square-wave amplitude for punchy head knocking.

### 2. Spindle Motor Sound (300 RPM Hum)
- **Frequency:** 110 Hz square/pulse wave.
- **Behavior:**
  - Active only when `SOUND_MOTOR_ON` is true AND `SoundSettings.motor_enabled` is true AND `volume != Off`.
  - Emitted as a continuous background tone with the selected volume duty cycle.
  - Temporarily suspended during stepper clicks to prevent phase interference and maximize click clarity.

### 3. Stepper Click & Head Knocking
- **Frequency:** 1.2 kHz pulse wave (matching authentic Pi1541 `SoundOnGPIOFreq`).
- **Normal Track Step:**
  - Duration: 1.8 ms.
  - Duty cycle: Scaled by master volume setting.
- **Head Bumping (Track 1 Stop Knock):**
  - Triggered when `SOUND_BUMPED_STOP` is asserted.
  - Duration: 2.8 ms with an accented duty cycle, simulating the physical rattle of the head assembly hitting the stop bumper.
- **Rapid Seek Handling:**
  - If multiple steps accumulate in `SOUND_STEP_COUNTER` during fastloader multi-track seeks, `SoundTask` executes successive clicks with a 0.5 ms inter-click gap, creating the authentic continuous "drrr-drrr-drrr" seek sound without lag.

## Data Structures & Configuration

### `SoundSettings` Structure
Stored in `crates/pico1541-common/src/sound.rs` (or `manager`):
```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SoundVolume {
    Off = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SoundSettings {
    pub volume: SoundVolume,
    pub motor_enabled: bool,
    pub stepper_enabled: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            volume: SoundVolume::Medium,
            motor_enabled: true,
            stepper_enabled: true,
        }
    }
}
```

## Settings Menu & Persistence

### 1. OLED Settings UI
In `crates/pico1541-manager/src/tasks/ui/settings.rs`:
- Add menu entry: `Sound Settings`.
- Submenu options:
  - `Volume: [Off | Low | Med | High]`
  - `Motor: [On | Off]`
  - `Stepper: [On | Off]`
  - `Back`
- Interacting with each option toggles or cycles values immediately and updates active sound settings.

### 2. Flash Persistence (LittleFS)
- Stored at `/settings/sound.dat` (3 bytes: volume, motor_enabled, stepper_enabled).
- `SettingsCommand::SaveSoundSettings(SoundSettings)` sent over `SETTINGS_CHANNEL` to `SettingsSaveTask`.
- On boot, `main.rs` loads `/settings/sound.dat` or applies defaults if file is missing.

## Error Handling & Failsafes
- **Corrupt/Missing Settings:** Gracefully fall back to `SoundSettings::default()`.
- **Zero Sound Overhead:** When `volume == SoundVolume::Off`, PWM hardware counter is stopped and no async timer loops run.
- **Cross-Core Safety:** Shared flags are accessed exclusively through atomic types (`AtomicU32`, `AtomicBool`) with `Ordering::Relaxed` or `Ordering::Acquire/Release`, ensuring strict absence of locks or contention.

## Testing Strategy
1. **Unit Tests (`pico1541-emulator`):**
   - Test that `DriveMechanics` detects PB0/PB1 phase changes and increments the step counter.
   - Test that outward steps hitting `MIN_HALF_TRACK` flag the bump state.
   - Test that motor on/off transitions update motor state.
2. **Unit Tests (`pico1541-manager`):**
   - Test `SoundSettings` binary serialization and deserialization.
   - Test `SettingsSaveTask` processing of `SaveSoundSettings`.
   - Test `SoundTask` step draining and motor transitions.
3. **Firmware Release Build:**
   - Compile `pico1541-rp2350` in `--release` mode.
   - Verify zero compiler warnings.
