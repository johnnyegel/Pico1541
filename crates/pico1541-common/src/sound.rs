use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum SoundVolume {
    Off = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl SoundVolume {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            _ => Self::Off,
        }
    }

    pub fn duty_percent(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Low => 5,
            Self::Medium => 20,
            Self::High => 50,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

impl SoundSettings {
    pub fn to_bytes(&self) -> [u8; 3] {
        [
            self.volume as u8,
            self.motor_enabled as u8,
            self.stepper_enabled as u8,
        ]
    }

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

pub static SOUND_STEP_COUNTER: AtomicU32 = AtomicU32::new(0);
pub static SOUND_BUMPED_STOP: AtomicBool = AtomicBool::new(false);
pub static SOUND_MOTOR_ON: AtomicBool = AtomicBool::new(false);

#[inline(always)]
pub fn notify_step(bumped: bool) {
    SOUND_STEP_COUNTER.fetch_add(1, Ordering::Relaxed);
    if bumped {
        SOUND_BUMPED_STOP.store(true, Ordering::Relaxed);
    }
}

#[inline(always)]
pub fn notify_motor(running: bool) {
    SOUND_MOTOR_ON.store(running, Ordering::Relaxed);
}

#[inline(always)]
pub fn get_and_clear_steps() -> (u32, bool) {
    let steps = SOUND_STEP_COUNTER.swap(0, Ordering::Relaxed);
    let bumped = SOUND_BUMPED_STOP.swap(false, Ordering::Relaxed);
    (steps, bumped)
}

#[inline(always)]
pub fn is_motor_running() -> bool {
    SOUND_MOTOR_ON.load(Ordering::Relaxed)
}
