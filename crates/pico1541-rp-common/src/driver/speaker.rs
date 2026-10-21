use embassy_rp::pac;
use embassy_rp::peripherals::{DMA_CH2, DMA_CH3};
use embassy_rp::pwm::{ChannelAPin, Config as PwmConfig, Pwm, Slice};
use embassy_rp::Peri;
use pico1541_common::motor_sample::{sample_to_pwm_duty, MOTOR_HUM_SAMPLE, MOTOR_SAMPLE_RATE};
use pico1541_common::sound::SoundVolume;
use pico1541_common::stepper_sample::{
    stepper_sample_to_pwm_duty, stepper_sample_to_pwm_duty_stopped, BUMP_SAMPLE, STEPPING_SAMPLE,
};

/// Ultrasonic PWM carrier frequency wrap rate (22,050 Hz @ 250 MHz system clock).
pub const MOTOR_PWM_TOP: u16 = 11337;

/// Sample playback rate in Hz matching the authentic 1541 spindle sample.
pub const MOTOR_SAMPLE_RATE_HZ: u32 = MOTOR_SAMPLE_RATE;

/// Total number of PCM audio samples in the circular spindle motor buffer.
pub const MOTOR_SAMPLE_COUNT: usize = MOTOR_HUM_SAMPLE.len();


/// DMA Channel 2 is used as Worker streaming samples to PWM Slice 1 Channel A.
const DMA_CH_WORKER: usize = 2;
/// DMA Channel 3 is used as Reloader resetting Channel 2 in an infinite circular loop.
const DMA_CH_RELOADER: usize = 3;

/// Abort bitmask for DMA channels 2 and 3: (1 << 2) | (1 << 3) = 0x000C.
const DMA_MOTOR_ABORT_MASK: u16 = (1 << DMA_CH_WORKER) | (1 << DMA_CH_RELOADER);

/// Channel enable bitmask (bit 0 in AL1_CTRL).
const DMA_CHANNEL_ENABLE_MASK: u32 = 1 << 0;

static mut MOTOR_DUTY_BUFFER: [u16; MOTOR_SAMPLE_COUNT] = [0; MOTOR_SAMPLE_COUNT];
static mut MOTOR_BUFFER_PTR: *const u16 = core::ptr::null();

pub const STEPPER_BUFFER_CAPACITY: usize = BUMP_SAMPLE.len();
static mut STEPPER_DUTY_BUFFER: [u16; STEPPER_BUFFER_CAPACITY] = [0; STEPPER_BUFFER_CAPACITY];

/// PWM Piezo audio driver for stepper clicks, tone generation, and spindle motor sound.
///
/// Sample DMA streaming specifically targets PWM Slice 1 Channel A (GPIO 2) and utilizes
/// hardware DMA Channels 2 & 3 in a circular ping-pong configuration.
pub struct PwmPiezo<'d> {
    pwm: Pwm<'d>,
    motor_humming: bool,
    current_volume: SoundVolume,
}

impl<'d> PwmPiezo<'d> {
    pub fn new<T: Slice>(
        slice: Peri<'d, T>,
        pin: Peri<'d, impl ChannelAPin<T>>,
        _dma_worker: Peri<'d, DMA_CH2>,
        _dma_reloader: Peri<'d, DMA_CH3>,
    ) -> Self {
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        let pwm = Pwm::new_output_a(slice, pin, config);
        Self {
            pwm,
            motor_humming: false,
            current_volume: SoundVolume::Off,
        }
    }

    /// Stops tone generation or motor hum, setting PWM compare and top to 0 and clearing humming state.
    pub fn stop(&mut self) {
        if self.motor_humming {
            Self::stop_dma();
        }
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        self.pwm.set_config(&config);
        self.motor_humming = false;
    }

    /// Set frequency in Hz and duty cycle percent (0-50%).
    /// If motor hum is active, temporarily pauses DMA before configuring the tone.
    pub fn play_tone(&mut self, freq_hz: u32, duty_percent: u8) {
        if freq_hz == 0 || duty_percent == 0 {
            self.stop();
            return;
        }

        if self.motor_humming {
            Self::stop_dma();
            self.motor_humming = false;
        }

        // System clock is 250 MHz or 125 MHz. Use divider 64 to fit 16-bit TOP.
        // For 110 Hz: 250_000_000 / (64 * 110) = 35_511 (fits in u16!)
        let divider: u8 = 64;
        let counter_hz = 250_000_000 / (divider as u32);
        let top = (counter_hz / freq_hz).clamp(1, 65535) as u16;
        let compare = (top as u32 * (duty_percent as u32) / 100).clamp(0, top as u32) as u16;

        let mut config = PwmConfig::default();
        config.divider = divider.into();
        config.top = top;
        config.compare_a = compare;
        self.pwm.set_config(&config);
    }

    /// Returns whether spindle motor hum sample streaming is currently active.
    pub fn is_motor_humming(&self) -> bool {
        self.motor_humming
    }

    /// Start authentic 1541 motor hum playback via circular DMA into PWM Slice 1 Channel A.
    ///
    /// Populates `MOTOR_DUTY_BUFFER` with scaled samples, sets the PWM carrier frequency to
    /// 22,050 Hz (TOP = 11,337 @ 250 MHz), and configures DMA Channels 2 & 3 in a ping-pong loop.
    pub fn start_motor_hum(&mut self, volume: SoundVolume) {
        if volume == SoundVolume::Off {
            self.stop_motor_hum();
            return;
        }

        if self.motor_humming {
            Self::stop_dma();
        }

        if volume != self.current_volume {
            Self::populate_duty_buffer(volume);
            self.current_volume = volume;
        }

        let mut config = PwmConfig::default();
        config.divider = 1u8.into();
        config.top = MOTOR_PWM_TOP;
        config.compare_a = 0;
        self.pwm.set_config(&config);

        Self::start_dma();
        self.motor_humming = true;
    }

    /// Stops spindle motor hum playback by disabling and aborting DMA channels 2 & 3,
    /// resetting PWM Slice 1 compare and top values to 0, and clearing `motor_humming`.
    pub fn stop_motor_hum(&mut self) {
        Self::stop_dma();
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        self.pwm.set_config(&config);
        self.motor_humming = false;
    }

    /// Updates the volume of the running motor hum in real-time.
    ///
    /// If volume is `Off`, DMA is stopped. Otherwise, the duty cycle values in `MOTOR_DUTY_BUFFER`
    /// are updated in place so the next circular DMA period plays at the new volume level.
    pub fn update_motor_volume(&mut self, volume: SoundVolume) {
        if volume == self.current_volume {
            return;
        }
        if volume == SoundVolume::Off {
            self.stop_motor_hum();
        } else {
            Self::populate_duty_buffer(volume);
            self.current_volume = volume;
        }
    }

    /// Restores ultrasonic carrier (22,050 Hz) and restarts DMA after playing a stepper click or head bump tone.
    #[inline]
    pub fn resume_motor_hum_after_tone(&mut self, volume: SoundVolume) {
        self.start_motor_hum(volume);
    }

    /// Updates the motor duty buffer to a specific envelope progress step (0..=max_steps).
    ///
    /// When step == 0 or volume == Off, stops DMA if humming, zeroes the buffer, and sets current_volume to Off.
    /// When step == max_steps, populates the buffer and sets current_volume to volume.
    /// When 0 < step < max_steps, populates the buffer and sets current_volume to Off.
    pub fn set_motor_envelope_step(&mut self, volume: SoundVolume, step: u8, max_steps: u8) {
        if volume == SoundVolume::Off || step == 0 {
            if self.motor_humming {
                Self::stop_dma();
                let mut config = PwmConfig::default();
                config.top = 0;
                config.compare_a = 0;
                self.pwm.set_config(&config);
                self.motor_humming = false;
            }
            Self::zero_duty_buffer();
            self.current_volume = SoundVolume::Off;
            return;
        }

        if step == max_steps {
            self.current_volume = volume;
        } else {
            self.current_volume = SoundVolume::Off;
        }

        let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
        unsafe {
            MOTOR_BUFFER_PTR = buf_ptr;
            for i in 0..MOTOR_SAMPLE_COUNT {
                *buf_ptr.add(i) = pico1541_common::motor_sample::sample_to_pwm_duty_faded(
                    MOTOR_HUM_SAMPLE[i],
                    MOTOR_PWM_TOP,
                    volume,
                    step,
                    max_steps,
                );
            }
        }
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);

        if !self.motor_humming {
            let mut config = PwmConfig::default();
            config.divider = 1u8.into();
            config.top = MOTOR_PWM_TOP;
            config.compare_a = 0;
            self.pwm.set_config(&config);

            Self::start_dma();
            self.motor_humming = true;
        }
    }

    /// Temporarily pauses Worker Channel 2 without resetting the chaining configuration.
    pub fn pause_motor_dma() {
        let dma = pac::DMA;
        dma.ch(DMA_CH_WORKER)
            .al1_ctrl()
            .modify(|w| *w &= !DMA_CHANNEL_ENABLE_MASK);
    }

    /// Resumes Worker Channel 2 streaming immediately.
    pub fn resume_motor_dma() {
        let dma = pac::DMA;
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
        dma.ch(DMA_CH_WORKER)
            .al1_ctrl()
            .modify(|w| *w |= DMA_CHANNEL_ENABLE_MASK);
    }

    fn populate_stepper_buffer(volume: SoundVolume, is_bump: bool, stopped: bool) -> usize {
        let buf_ptr = core::ptr::addr_of_mut!(STEPPER_DUTY_BUFFER) as *mut u16;
        let len = if is_bump {
            BUMP_SAMPLE.len()
        } else {
            STEPPING_SAMPLE.len()
        };

        unsafe {
            if stopped {
                for i in 0..len {
                    let sample = if is_bump {
                        BUMP_SAMPLE[i]
                    } else {
                        STEPPING_SAMPLE[i]
                    };
                    *buf_ptr.add(i) = stepper_sample_to_pwm_duty_stopped(
                        sample,
                        MOTOR_PWM_TOP,
                        volume,
                        i,
                        len,
                    );
                }
            } else {
                for i in 0..len {
                    let sample = if is_bump {
                        BUMP_SAMPLE[i]
                    } else {
                        STEPPING_SAMPLE[i]
                    };
                    *buf_ptr.add(i) = stepper_sample_to_pwm_duty(
                        sample,
                        MOTOR_PWM_TOP,
                        volume,
                    );
                }
            }
        }
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
        len
    }

    /// Plays an authentic mechanical stepper or bump sample while the spindle motor is active.
    ///
    /// Temporarily points Worker DMA Channel 2 to `STEPPER_DUTY_BUFFER`, streaming the 31.3 ms
    /// step (or 118.6 ms bump) at 22,050 Hz without dropping the ultrasonic carrier or 1.65V DC center.
    /// When the sample completes, seamlessly restores circular motor hum streaming.
    pub async fn play_active_step_click(&mut self, volume: SoundVolume, is_bump: bool) {
        if !self.motor_humming || volume == SoundVolume::Off {
            return;
        }

        let len = Self::populate_stepper_buffer(volume, is_bump, false);
        Self::pause_motor_dma();

        let dma = pac::DMA;
        let ch_worker = dma.ch(DMA_CH_WORKER);

        // Configure Worker Channel 2 to stream the stepper buffer (unchained):
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

        // Await sample playback completion: 691 samples ≈ 31.3 ms (32 ms), 2616 samples ≈ 118.6 ms (120 ms)
        let duration_ms = if is_bump { 120 } else { 32 };
        embassy_time::Timer::after(embassy_time::Duration::from_millis(duration_ms)).await;

        // Restore Worker Channel 2 to circular motor hum buffer
        Self::pause_motor_dma();
        ch_worker
            .read_addr()
            .write_value(core::ptr::addr_of!(MOTOR_DUTY_BUFFER) as u32);
        ch_worker
            .trans_count()
            .write_value(pac::dma::regs::ChTransCount(MOTOR_SAMPLE_COUNT as u32));

        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
        ch_worker.ctrl_trig().modify(|w| {
            w.set_chain_to(DMA_CH_RELOADER as u8);
            w.set_en(true);
        });
    }

    /// Plays an authentic mechanical stepper or bump sample when the spindle motor is stopped.
    ///
    /// Sets the PWM carrier to 22,050 Hz and streams the DC-ramped sample via one-shot DMA.
    /// The sub-audible 1.1 ms ramp at the start and end prevents capacitive clicks, delivering
    /// a crisp, authentic mechanical click of the head stepping while the spindle is parked.
    pub async fn play_stopped_step_click(&mut self, volume: SoundVolume, is_bump: bool) {
        if volume == SoundVolume::Off {
            return;
        }

        let len = Self::populate_stepper_buffer(volume, is_bump, true);

        // Configure PWM slice for 22,050 Hz carrier
        let mut config = PwmConfig::default();
        config.divider = 1u8.into();
        config.top = MOTOR_PWM_TOP;
        config.compare_a = 0;
        self.pwm.set_config(&config);

        let dma = pac::DMA;
        let ch_worker = dma.ch(DMA_CH_WORKER);

        ch_worker
            .write_addr()
            .write_value(pac::PWM.ch(1).cc().as_ptr() as u32);
        ch_worker
            .read_addr()
            .write_value(core::ptr::addr_of!(STEPPER_DUTY_BUFFER) as u32);
        ch_worker
            .trans_count()
            .write_value(pac::dma::regs::ChTransCount(len as u32));

        let mut ctrl_worker = ch_worker.ctrl_trig().read();
        ctrl_worker.set_data_size(pac::dma::vals::DataSize::SIZE_HALFWORD);
        ctrl_worker.set_incr_read(true);
        ctrl_worker.set_incr_write(false);
        ctrl_worker.set_treq_sel(pac::dma::vals::TreqSel::PWM_WRAP1);
        ctrl_worker.set_chain_to(DMA_CH_WORKER as u8);
        ctrl_worker.set_ring_sel(false);
        ctrl_worker.set_ring_size(0);
        ctrl_worker.set_en(false);
        ch_worker.ctrl_trig().write_value(ctrl_worker);

        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
        ch_worker.ctrl_trig().modify(|w| w.set_en(true));

        let duration_ms = if is_bump { 120 } else { 32 };
        embassy_time::Timer::after(embassy_time::Duration::from_millis(duration_ms)).await;

        // Silence PWM and disable DMA channel
        Self::pause_motor_dma();
        self.stop();
    }


    fn zero_duty_buffer() {
        let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
        unsafe {
            for i in 0..MOTOR_SAMPLE_COUNT {
                *buf_ptr.add(i) = 0;
            }
        }
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    }

    fn populate_duty_buffer(volume: SoundVolume) {
        let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
        unsafe {
            MOTOR_BUFFER_PTR = buf_ptr;
            for i in 0..MOTOR_SAMPLE_COUNT {
                *buf_ptr.add(i) = sample_to_pwm_duty(MOTOR_HUM_SAMPLE[i], MOTOR_PWM_TOP, volume);
            }
        }
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);
    }

    fn stop_dma() {
        let dma = pac::DMA;
        // 1. Clear EN on Reloader (ch 3) and Worker (ch 2)
        dma.ch(DMA_CH_RELOADER)
            .al1_ctrl()
            .modify(|w| *w &= !DMA_CHANNEL_ENABLE_MASK);
        dma.ch(DMA_CH_WORKER)
            .al1_ctrl()
            .modify(|w| *w &= !DMA_CHANNEL_ENABLE_MASK);

        // 2. Abort both channels simultaneously
        dma.chan_abort()
            .write(|w| w.set_chan_abort(DMA_MOTOR_ABORT_MASK));
        while dma.chan_abort().read().chan_abort() & DMA_MOTOR_ABORT_MASK != 0 {}
    }

    fn start_dma() {
        let dma = pac::DMA;

        // Ensure no previous abort is in progress
        while dma.chan_abort().read().chan_abort() & DMA_MOTOR_ABORT_MASK != 0 {}

        let ch_worker = dma.ch(DMA_CH_WORKER);
        let ch_reloader = dma.ch(DMA_CH_RELOADER);

        // Reloader Channel (Ch 3):
        ch_reloader
            .read_addr()
            .write_value(core::ptr::addr_of!(MOTOR_BUFFER_PTR) as u32);
        ch_reloader
            .write_addr()
            .write_value(ch_worker.al3_read_addr_trig().as_ptr() as u32);
        ch_reloader
            .trans_count()
            .write_value(pac::dma::regs::ChTransCount(1));

        let mut ctrl_reloader = ch_reloader.ctrl_trig().read();
        ctrl_reloader.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_reloader.set_incr_read(false);
        ctrl_reloader.set_incr_write(false);
        ctrl_reloader.set_treq_sel(pac::dma::vals::TreqSel::PERMANENT);
        ctrl_reloader.set_chain_to(DMA_CH_RELOADER as u8);
        ctrl_reloader.set_ring_sel(false);
        ctrl_reloader.set_ring_size(0);
        ctrl_reloader.set_en(false);
        ch_reloader.ctrl_trig().write_value(ctrl_reloader);

        // Worker Channel (Ch 2):
        ch_worker
            .write_addr()
            .write_value(pac::PWM.ch(1).cc().as_ptr() as u32);
        ch_worker
            .read_addr()
            .write_value(core::ptr::addr_of!(MOTOR_DUTY_BUFFER) as u32);
        ch_worker
            .trans_count()
            .write_value(pac::dma::regs::ChTransCount(MOTOR_SAMPLE_COUNT as u32));

        let mut ctrl_worker = ch_worker.ctrl_trig().read();
        ctrl_worker.set_data_size(pac::dma::vals::DataSize::SIZE_HALFWORD);
        ctrl_worker.set_incr_read(true);
        ctrl_worker.set_incr_write(false);
        ctrl_worker.set_treq_sel(pac::dma::vals::TreqSel::PWM_WRAP1);
        ctrl_worker.set_chain_to(DMA_CH_RELOADER as u8);
        ctrl_worker.set_ring_sel(false);
        ctrl_worker.set_ring_size(0);
        ctrl_worker.set_en(false);
        ch_worker.ctrl_trig().write_value(ctrl_worker);

        // Enable Reloader Channel (Ch 3) without triggering:
        ch_reloader
            .al1_ctrl()
            .modify(|w| *w |= DMA_CHANNEL_ENABLE_MASK);

        // Ensure all buffer and register writes are committed prior to DMA trigger
        cortex_m::asm::dmb();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::Release);

        // Enable and trigger Worker Channel (Ch 2):
        ch_worker.ctrl_trig().modify(|w| w.set_en(true));
    }
}
