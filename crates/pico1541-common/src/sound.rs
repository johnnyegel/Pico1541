//! 1541 Drive Mechanical Sound Emulation Settings & Inter-Core Notification State.
//!
//! # Architecture
//! - **Core 1 (Producer / Emulator)**: Drives the 6502 and mechanics. When stepper motor phases
//!   change or the spindle motor is toggled, Core 1 calls [`notify_step`] and [`notify_motor`].
//!   These operations use relaxed atomics to guarantee zero blocking or cycle jitter.
//! - **Core 0 (Consumer / Manager)**: Runs Embassy async tasks, including the sound synthesis task.
//!   Core 0 periodically polls [`get_and_clear_steps`] and [`is_motor_running`] to modulate
//!   hardware PWM audio output accordingly.

use portable_atomic::{AtomicBool, AtomicU32, Ordering};

/// Master sound volume level.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum SoundVolume {
    /// Sound disabled (0% duty cycle).
    Off = 0,
    /// Low volume (5% duty cycle).
    Low = 1,
    /// Medium volume (20% duty cycle).
    Medium = 2,
    /// High volume (50% duty cycle).
    High = 3,
}

impl SoundVolume {
    /// Cycles to the next volume level in order: Off -> Low -> Medium -> High -> Off.
    pub const fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Off,
        }
    }

    /// Converts a raw byte to a [`SoundVolume`].
    /// Any value outside `1..=3` defaults to [`SoundVolume::Off`].
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            _ => Self::Off,
        }
    }

    /// Returns the target PWM duty cycle percentage (0-50%) for this volume level.
    pub const fn duty_percent(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Low => 5,
            Self::Medium => 20,
            Self::High => 50,
        }
    }
}

/// Sound emulation configuration settings persisted in flash / LittleFS.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SoundSettings {
    /// Master volume level.
    pub volume: SoundVolume,
    /// Whether spindle motor hum sound is enabled.
    pub motor_enabled: bool,
    /// Whether stepper motor clatter and head-bumping knock sounds are enabled.
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

impl SoundSettings {
    /// Serializes settings to a 3-byte array (`[volume, motor_enabled, stepper_enabled]`).
    pub const fn to_bytes(&self) -> [u8; 3] {
        [
            self.volume as u8,
            self.motor_enabled as u8,
            self.stepper_enabled as u8,
        ]
    }

    /// Deserializes settings from a byte slice. Returns `None` if slice contains fewer than 3 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 3 {
            return None;
        }
        Some(Self {
            volume: SoundVolume::from_u8(bytes[0]),
            motor_enabled: bytes[1] != 0,
            stepper_enabled: bytes[2] != 0,
        })
    }
}

/// Accumulated stepper motor step events (written by Core 1, cleared by Core 0).
pub static SOUND_STEP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Set to true if any pending step attempted to collide against the track 1 physical stop.
pub static SOUND_BUMPED_STOP: AtomicBool = AtomicBool::new(false);

/// Current state of the spindle motor (true if running).
pub static SOUND_MOTOR_ON: AtomicBool = AtomicBool::new(false);

/// Core 1 Producer: Notify that a stepper step occurred.
///
/// If `bumped` is true, indicates collision with track 1 physical stop (head knocking).
/// The `bumped` flag is stored before incrementing `SOUND_STEP_COUNTER` so the consumer on
/// Core 0 never observes an incremented step count without the accompanying collision flag.
#[inline(always)]
pub fn notify_step(bumped: bool) {
    if bumped {
        SOUND_BUMPED_STOP.store(true, Ordering::Relaxed);
    }
    SOUND_STEP_COUNTER.fetch_add(1, Ordering::Relaxed);
}

/// Core 1 Producer: Notify that the spindle motor running state changed.
#[inline(always)]
pub fn notify_motor(running: bool) {
    SOUND_MOTOR_ON.store(running, Ordering::Relaxed);
}

/// Core 0 Consumer: Atomically drain accumulated steps and clear the head-bump collision flag.
///
/// Returns `(step_count, bumped)`.
#[inline(always)]
pub fn get_and_clear_steps() -> (u32, bool) {
    let steps = SOUND_STEP_COUNTER.swap(0, Ordering::Relaxed);
    let bumped = SOUND_BUMPED_STOP.swap(false, Ordering::Relaxed);
    (steps, bumped)
}

/// Core 0 Consumer: Query whether the spindle motor is currently running.
#[inline(always)]
pub fn is_motor_running() -> bool {
    SOUND_MOTOR_ON.load(Ordering::Relaxed)
}
