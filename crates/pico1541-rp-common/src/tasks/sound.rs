use crate::driver::speaker::PwmPiezo;
use embassy_time::{Duration, Timer};
use pico1541_common::sound::{get_and_clear_steps, is_motor_running, SoundSettings, SoundVolume};
use portable_atomic::{AtomicBool, AtomicU8, Ordering};

pub static CURRENT_SOUND_SETTINGS: SoundSettingsCell = SoundSettingsCell::new();

pub struct SoundSettingsCell {
    volume: AtomicU8,
    motor: AtomicBool,
    stepper: AtomicBool,
}

impl SoundSettingsCell {
    pub const fn new() -> Self {
        Self {
            volume: AtomicU8::new(SoundVolume::Medium as u8),
            motor: AtomicBool::new(true),
            stepper: AtomicBool::new(true),
        }
    }

    pub fn get(&self) -> SoundSettings {
        SoundSettings {
            volume: SoundVolume::from_u8(self.volume.load(Ordering::Relaxed)),
            motor_enabled: self.motor.load(Ordering::Relaxed),
            stepper_enabled: self.stepper.load(Ordering::Relaxed),
        }
    }

    pub fn set(&self, settings: SoundSettings) {
        self.volume.store(settings.volume as u8, Ordering::Relaxed);
        self.motor.store(settings.motor_enabled, Ordering::Relaxed);
        self.stepper.store(settings.stepper_enabled, Ordering::Relaxed);
    }
}

#[embassy_executor::task]
pub async fn sound_task(mut piezo: PwmPiezo<'static>) {
    let mut playing_motor = false;

    loop {
        let settings = CURRENT_SOUND_SETTINGS.get();
        let volume_percent = settings.volume.duty_percent();

        if volume_percent == 0 {
            if playing_motor {
                piezo.stop();
                playing_motor = false;
            }
            Timer::after(Duration::from_millis(10)).await;
            continue;
        }

        let (steps, bumped) = get_and_clear_steps();

        if steps > 0 && settings.stepper_enabled {
            if playing_motor {
                piezo.stop();
                playing_motor = false;
            }
            // Play stepping clicks
            for i in 0..steps.min(10) {
                let duration_us = if bumped && i == 0 { 2800 } else { 1800 };
                piezo.play_tone(1200, volume_percent);
                Timer::after(Duration::from_micros(duration_us)).await;

                piezo.stop();
                if steps > 1 {
                    Timer::after(Duration::from_micros(500)).await;
                }
            }
        } else if is_motor_running() && settings.motor_enabled {
            if !playing_motor {
                piezo.play_tone(110, volume_percent / 2); // Soft low-frequency hum
                playing_motor = true;
            }
            Timer::after(Duration::from_millis(2)).await;
        } else {
            if playing_motor {
                piezo.stop();
                playing_motor = false;
            }
            Timer::after(Duration::from_millis(2)).await;
        }
    }
}
