use embassy_rp::pac;
use embassy_rp::pwm::{ChannelAPin, Config as PwmConfig, Pwm, Slice};
use embassy_rp::Peri;
use pico1541_common::motor_sample::{sample_to_pwm_duty, MOTOR_HUM_SAMPLE, MOTOR_SAMPLE_RATE};
use pico1541_common::sound::SoundVolume;

/// Ultrasonic PWM carrier frequency wrap rate (22,050 Hz @ 250 MHz system clock).
pub const MOTOR_PWM_TOP: u16 = 11337;

/// Sample playback rate in Hz matching the authentic 1541 spindle sample.
pub const MOTOR_SAMPLE_RATE_HZ: u32 = MOTOR_SAMPLE_RATE;

/// DMA Channel 2 is used as Worker streaming samples to PWM Slice 1 Channel A.
const DMA_CH_WORKER: usize = 2;
/// DMA Channel 3 is used as Reloader resetting Channel 2 in an infinite circular loop.
const DMA_CH_RELOADER: usize = 3;

/// Abort bitmask for DMA channels 2 and 3: (1 << 2) | (1 << 3) = 0x000C.
const DMA_MOTOR_ABORT_MASK: u16 = (1 << DMA_CH_WORKER) | (1 << DMA_CH_RELOADER);

/// Channel enable bitmask (bit 0 in AL1_CTRL).
const DMA_CHANNEL_ENABLE_MASK: u32 = 1 << 0;

static mut MOTOR_DUTY_BUFFER: [u16; 8819] = [0; 8819];
static mut MOTOR_BUFFER_PTR: *const u16 = core::ptr::null();

pub struct PwmPiezo<'d> {
    pwm: Pwm<'d>,
    motor_humming: bool,
}

impl<'d> PwmPiezo<'d> {
    pub fn new<T: Slice>(
        slice: Peri<'d, T>,
        pin: Peri<'d, impl ChannelAPin<T>>,
    ) -> Self {
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        let pwm = Pwm::new_output_a(slice, pin, config);
        Self {
            pwm,
            motor_humming: false,
        }
    }

    /// Stops tone generation. If motor hum was active, pauses DMA so tone/silence can take effect.
    pub fn stop(&mut self) {
        if self.motor_humming {
            Self::stop_dma();
        }
        let mut config = PwmConfig::default();
        config.top = 0;
        config.compare_a = 0;
        self.pwm.set_config(&config);
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
            return;
        }

        if self.motor_humming {
            Self::stop_dma();
        }

        let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
        unsafe {
            MOTOR_BUFFER_PTR = buf_ptr;
            for i in 0..8819 {
                *buf_ptr.add(i) = sample_to_pwm_duty(MOTOR_HUM_SAMPLE[i], MOTOR_PWM_TOP, volume);
            }
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
        if volume == SoundVolume::Off {
            self.stop_motor_hum();
        } else {
            let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
            unsafe {
                MOTOR_BUFFER_PTR = buf_ptr;
                for i in 0..8819 {
                    *buf_ptr.add(i) = sample_to_pwm_duty(MOTOR_HUM_SAMPLE[i], MOTOR_PWM_TOP, volume);
                }
            }
        }
    }

    /// Restores ultrasonic carrier (22,050 Hz) and restarts DMA after playing a stepper click or head bump tone.
    pub fn resume_motor_hum_after_tone(&mut self, volume: SoundVolume) {
        if !self.motor_humming {
            return;
        }

        if volume == SoundVolume::Off {
            self.stop_motor_hum();
            return;
        }

        let buf_ptr = core::ptr::addr_of_mut!(MOTOR_DUTY_BUFFER) as *mut u16;
        unsafe {
            MOTOR_BUFFER_PTR = buf_ptr;
            for i in 0..8819 {
                *buf_ptr.add(i) = sample_to_pwm_duty(MOTOR_HUM_SAMPLE[i], MOTOR_PWM_TOP, volume);
            }
        }

        let mut config = PwmConfig::default();
        config.divider = 1u8.into();
        config.top = MOTOR_PWM_TOP;
        config.compare_a = 0;
        self.pwm.set_config(&config);

        Self::start_dma();
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
            .write_value(pac::dma::regs::ChTransCount(8819));

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

        // Enable and trigger Worker Channel (Ch 2):
        ch_worker.ctrl_trig().modify(|w| w.set_en(true));
    }
}
