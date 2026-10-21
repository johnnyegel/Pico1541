use crate::driver::speaker::{
    PwmPiezo, STEPPER_BUMP_DURATION_US, STEPPER_CLICK_DURATION_US, STEPPER_CLICK_FREQ_HZ,
};
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

const FADE_STEPS: u8 = 15;
const FADE_STEP_DURATION_MS: u64 = 10;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MotorEnvelope {
    Stopped,
    FadingIn { step: u8 },
    Running,
    FadingOut { step: u8 },
}

#[embassy_executor::task]
pub async fn sound_task(mut piezo: PwmPiezo<'static>) {
    let mut envelope = MotorEnvelope::Stopped;
    let mut last_volume = CURRENT_SOUND_SETTINGS.get().volume;

    loop {
        let settings = CURRENT_SOUND_SETTINGS.get();
        let volume_percent = settings.volume.duty_percent();

        // 1. Muted Volume Handling
        if volume_percent == 0 {
            if envelope != MotorEnvelope::Stopped {
                piezo.stop_motor_hum();
                envelope = MotorEnvelope::Stopped;
            }
            let _ = get_and_clear_steps();
            Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
            continue;
        }

        // 2. Stepper Click Handling
        let (steps, bumped) = get_and_clear_steps();
        if steps > 0 && settings.stepper_enabled {
            for i in 0..steps.min(10) {
                let is_bump = bumped && i == 0;
                if envelope == MotorEnvelope::Stopped {
                    // Motor stopped: standard tone click
                    let duration_us = if is_bump {
                        STEPPER_BUMP_DURATION_US
                    } else {
                        STEPPER_CLICK_DURATION_US
                    };
                    piezo.play_tone(STEPPER_CLICK_FREQ_HZ, volume_percent);
                    Timer::after(Duration::from_micros(duration_us)).await;
                    piezo.stop();
                } else {
                    // Motor active: carrier-locked click without dropping DC bias or restarting DMA
                    piezo.play_active_step_click(settings.volume, is_bump).await;
                }

                if steps > 1 {
                    Timer::after(Duration::from_micros(500)).await;
                }
            }
        }

        // 3. Spindle Motor Envelope State Machine
        let motor_should_run = is_motor_running() && settings.motor_enabled;

        // Handle live volume change while running
        if settings.volume != last_volume {
            if envelope == MotorEnvelope::Running {
                piezo.update_motor_volume(settings.volume);
            }
            last_volume = settings.volume;
        }

        match envelope {
            MotorEnvelope::Stopped => {
                if motor_should_run {
                    piezo.set_motor_envelope_step(settings.volume, 1, FADE_STEPS);
                    envelope = MotorEnvelope::FadingIn { step: 1 };
                }
                Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
            }
            MotorEnvelope::FadingIn { step } => {
                if !motor_should_run {
                    // Motor stopped mid-fade-in: reverse to fading out
                    envelope = MotorEnvelope::FadingOut { step };
                } else if step >= FADE_STEPS {
                    envelope = MotorEnvelope::Running;
                } else {
                    let next_step = step + 1;
                    piezo.set_motor_envelope_step(settings.volume, next_step, FADE_STEPS);
                    envelope = MotorEnvelope::FadingIn { step: next_step };
                }
                Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
            }
            MotorEnvelope::Running => {
                if !motor_should_run {
                    piezo.set_motor_envelope_step(settings.volume, FADE_STEPS - 1, FADE_STEPS);
                    envelope = MotorEnvelope::FadingOut { step: FADE_STEPS - 1 };
                    Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
                } else {
                    Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
                }
            }
            MotorEnvelope::FadingOut { step } => {
                if motor_should_run {
                    // Motor re-triggered mid-fade-out: reverse to fading in
                    envelope = MotorEnvelope::FadingIn { step };
                    Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
                } else if step <= 1 {
                    piezo.set_motor_envelope_step(settings.volume, 0, FADE_STEPS);
                    envelope = MotorEnvelope::Stopped;
                    Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
                } else {
                    let next_step = step - 1;
                    piezo.set_motor_envelope_step(settings.volume, next_step, FADE_STEPS);
                    envelope = MotorEnvelope::FadingOut { step: next_step };
                    Timer::after(Duration::from_millis(FADE_STEP_DURATION_MS)).await;
                }
            }
        }
    }
}
