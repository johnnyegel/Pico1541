use pico1541_common::magnetic_surface::MagneticSurface;
use pico1541_common::track_buffer::{TrackBuffer, TrackLayout};
use embassy_rp::pac;


// Speed zone PIO timing
// The PIO program executes exactly 8 cycles per *bit*, not per byte!
const PIO_CYCLES_PER_BIT: u64 = 8; 
// We use exactly 100M for perfect real-time emulation.
const PIO_TIMING_DENOMINATOR: u64 = 100_000_000 * PIO_CYCLES_PER_BIT;

const GPIO_SYNC_PIN: u8 = 10; // GPIO 10 is the SYNC pin for the PIO program

// This const function executes entirely at compile time.
const fn calc_speed_zone_register(multiplier: u64) -> u32 {
    // 1. Calculate the scaled fraction: (Clock * Time * 256) / Denominator
    // 64-bit math guarantees no overflow during the massive multiplication step.
    let div_256 = (pico1541_common::SYSTEM_CLOCK_FREQ as u64 * multiplier * 256) / PIO_TIMING_DENOMINATOR;
    
    // Cast back to u32 for the hardware registers safely
    div_256 as u32
}

const PIO_SPEED_ZONES: [u32; 4] = [
    calc_speed_zone_register((pico1541_common::track_buffer::speed_zone_bit_period_100ps(0) / 100) as u64),
    calc_speed_zone_register((pico1541_common::track_buffer::speed_zone_bit_period_100ps(1) / 100) as u64),
    calc_speed_zone_register((pico1541_common::track_buffer::speed_zone_bit_period_100ps(2) / 100) as u64),
    calc_speed_zone_register((pico1541_common::track_buffer::speed_zone_bit_period_100ps(3) / 100) as u64),
];

// Static buffers for DMA
static mut CURRENT_TRACK_PTR: u32 = 0;

pub struct PioMagneticSurface {
    is_motor_on: bool,
    stream_length: u32,
    motor_start_time_us: u32,
    saved_angle_us: u32,

    _sync_pin: embassy_rp::pio::Pin<'static, embassy_rp::peripherals::PIO0>,
    _loaded_prg: embassy_rp::pio::LoadedProgram<'static, embassy_rp::peripherals::PIO0>,
    _sm: embassy_rp::pio::StateMachine<'static, embassy_rp::peripherals::PIO0, 0>,
    
    current_speed_zone: u8,
}

impl PioMagneticSurface {
    const DMA_FLOPPY_ABORT_MASK: u16 = (1 << 0) | (1 << 1);
    const DMA_CHANNEL_ENABLE_MASK: u32 = (1 << 0);


    pub fn new(
        mut pio: embassy_rp::pio::Pio<'static, embassy_rp::peripherals::PIO0>,
    ) -> Self {
        let prg = crate::pio::magnetic_surface_program();
        let loaded_prg = pio.common.load_program(&prg);
        
        let mut sm = pio.sm0;
        let mut cfg = embassy_rp::pio::Config::default();
        cfg.clock_divider = 162u8.into(); // Overridden by set_speed_zone
        
        // PIO Setup according to user request
        cfg.shift_in.auto_fill = true; // We use ISR Autopush = true
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false; // OSR Autopull = false
        cfg.shift_out.threshold = 32; 
        cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Left;

        // SET pin mapping to GPIO 10
        let sync_pin = pio.common.make_pio_pin(unsafe { embassy_rp::peripherals::PIN_10::steal() });
        cfg.use_program(&loaded_prg, &[]);
        cfg.set_out_pins(&[&sync_pin]);
        cfg.set_set_pins(&[&sync_pin]);
        sm.set_config(&cfg);
        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sync_pin]);
        sm.set_enable(true);

        Self::init_1541_dma_channels();

        Self {
            is_motor_on: false,
            stream_length: 1, // Initialize to 1 to avoid divide by zero!
            motor_start_time_us: 0,
            saved_angle_us: 0,
            _sync_pin: sync_pin,
            _loaded_prg: loaded_prg,
            _sm: sm,
            current_speed_zone: 0,
        }
    }

    fn init_1541_dma_channels() {
        let dma = pac::DMA;
        let pio = pac::PIO0;

        let ch_a = dma.ch(0);

        // --- Channel A: TX Worker (Streams data to PIO) ---
        // Paced by PIO TX DREQ, 32-bit words, read inc, write no inc. Chains to B.
        ch_a.write_addr().write_value(pio.txf(0).as_ptr() as u32);
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_data_size(pac::dma::vals::DataSize::SIZE_BYTE);
        ctrl_a.set_incr_read(true);
        ctrl_a.set_incr_write(false);
        ctrl_a.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_a.set_chain_to(1); // Chain to Channel B
        ctrl_a.set_ring_sel(false);
        ctrl_a.set_ring_size(0);
        ctrl_a.set_en(false);
        ch_a.ctrl_trig().write_value(ctrl_a);

        // --- Channel B: TX Reloader (The Infinite Loop Engine) ---
        // Unpaced, 32-bit, no inc, trans_count 1, writes to ch_a AL3_READ_ADDR_TRIG.
        // DO NOT TOUCH THIS DMA CHAINING LOGIC!!!
        // THIS IS PERFECTLY CORRECT. THE RP2040 TRANS_COUNT AUTOMATICALLY RELOADS.
        // THIS CHANNEL PERFECTLY RESETS CHANNEL A INTO AN INFINITE LOOP
        // WITHOUT NEEDING ANY CPU INTERVENTION.
        // DO NOT TINKER WITH THIS, OR BREAK THIS, EVER AGAIN!!!
        let ch_b = dma.ch(1);
        ch_b.read_addr().write_value(core::ptr::addr_of!(CURRENT_TRACK_PTR) as u32);
        ch_b.write_addr().write_value(ch_a.al3_read_addr_trig().as_ptr() as u32);
        ch_b.trans_count().write_value(1);
        
        let mut ctrl_b = ch_b.ctrl_trig().read();
        ctrl_b.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_b.set_incr_read(false);
        ctrl_b.set_incr_write(false);
        ctrl_b.set_treq_sel(pac::dma::vals::TreqSel::PERMANENT);
        ctrl_b.set_chain_to(1); // Doesn't chain to others (chains to itself)
        ctrl_b.set_en(false);
        ch_b.ctrl_trig().write_value(ctrl_b);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn is_sync_active() -> bool {
        let pio = pac::PIO0;
        // Read internal PIO DBG_PADOUT register rather than physical GPIO
        (pio.dbg_padout().read() & (1 << GPIO_SYNC_PIN)) != 0
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn try_read_ram_byte(&mut self) -> Option<u8> {
        let pio = pac::PIO0;
        // Check if the RX FIFO is NOT empty (this is our exact BYTE_READY signal!)
        if (pio.fstat().read().rxempty() & (1 << 0)) == 0 {
            // Read the actual data directly from the PIO hardware FIFO
            let byte = pio.rxf(0).read() as u8;
            Some(byte)
        } else {
            None
        }
    }

    /// Asserts both floppy spinning DMA channels to abort any in-flight transfers.
    /// WARNING! This function does NOT wait for the channels to finish aborting. 
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn stop_floppy_dma() {
        let dma = pac::DMA;

        // 1. SAFE DISABLE: Clear Bit 0 (EN) using bitwise AND NOT  
        dma.ch(1).al1_ctrl().modify(|w| *w &= !Self::DMA_CHANNEL_ENABLE_MASK);
        dma.ch(0).al1_ctrl().modify(|w| *w &= !Self::DMA_CHANNEL_ENABLE_MASK);
        
        // 2. Abort both channels SIMULTANEOUSLY using the bitmask
        dma.chan_abort().write(|w| w.set_chan_abort(Self::DMA_FLOPPY_ABORT_MASK));
    }

    /// Starts the floppy spinning DMA channels.
    /// If the DMA has previously been stopped, the function will wait for the channels to
    /// finish aborting before re-enabling them.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn start_floppy_dma() {
        let dma = pac::DMA;

        // :: Ensure no DMA channels are not aborted
        while dma.chan_abort().read().chan_abort() & Self::DMA_FLOPPY_ABORT_MASK != 0 {}

        // :: Re-enable both channels
        // Simply set CH 1 enable flag, but DO NOT trigger it.
        dma.ch(1).al1_ctrl().modify(|w| *w |= Self::DMA_CHANNEL_ENABLE_MASK);

        // Trigger CH 0 enable flag, which will start the DMA trigger chaining
        dma.ch(0).ctrl_trig().modify(|w| w.set_en(true));
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn clear_pio_fifos() {
        let pio = pac::PIO0;
        let shiftctrl = pio.sm(0).shiftctrl();
        shiftctrl.modify(|w| w.set_fjoin_rx(!w.fjoin_rx()));
        shiftctrl.modify(|w| w.set_fjoin_rx(!w.fjoin_rx()));

        pio.ctrl().write(|w| {
            w.set_sm_restart(1 << 0);
            w.set_sm_enable(1 << 0);
        });
    }


    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_stream_source_offset(&self, stream: &'static TrackBuffer, offset: u32) {
        // :: Ensure the floppy DMA channels are stopped and the PIO FIFOs are cleared.
        Self::stop_floppy_dma();
        Self::clear_pio_fifos();

        // :: Set the new track pointer and length for the DMA transfer    
        let track_ptr = stream.data.as_ptr() as u32;
        let track_length = stream.length as u32;

        unsafe {
            // Store the unmodified track pointer to our reset DMA channel.
            CURRENT_TRACK_PTR = track_ptr;
        }

        // :: Realign the track for fun and benefit
        let mut adjusted_track_ptr = track_ptr;
        let mut adjusted_track_length = track_length;

        // Prevent buffer overflow, which can crash the drive emulation
        let safe_offset = if offset < track_length {
            offset
        } else {
            offset - track_length    
        };

        // Adjust the track_ptr and length to adjust to the start byte index
        adjusted_track_ptr += safe_offset;
        adjusted_track_length -= safe_offset;

        // :: Reconfigure the track pointer and length for the new track
        let dma_ch0 = pac::DMA.ch(0);
        dma_ch0.read_addr().write_value(adjusted_track_ptr);
        dma_ch0.trans_count().write_value(adjusted_track_length);
        
        // If the motor is on, we must apply the new track/noise configuration immediately
        if self.is_motor_on {
            // :: Re-enable the DMA channels to start the new transfer
            Self::start_floppy_dma();

            // Since the DMA is now triggered, we can immediately reset the length.
            // When the track end is reached, the new length will be loaded and used.
            // If the DMA is at the very end of the track when this happens, there is
            // a miniscule chance of a wrap of a few bytes. But this is expected to be
            // garbage anyway, so it is likely completely safe-
            dma_ch0.trans_count().write_value(track_length);
        }
    }
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn update_and_get_angle(&mut self, master_clock: u32) -> u32 {
        if self.is_motor_on {
            let delta = master_clock.wrapping_sub(self.motor_start_time_us);
            self.saved_angle_us = (self.saved_angle_us + delta) % 200_000;
            self.motor_start_time_us = master_clock;
        }
        self.saved_angle_us
    }
}

impl MagneticSurface for PioMagneticSurface {

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn is_sync_active(&self) -> bool {
        Self::is_sync_active()
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn try_read_byte(&mut self, _delta_cycles: u32) -> Option<u8> {
        self.try_read_ram_byte()
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        let dma = pac::DMA;
        let remaining = dma.ch(0).trans_count().read();
        self.stream_length.saturating_sub(remaining)
    }


    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_motor_running(&mut self, is_running: bool, master_clock: u32) {
        if self.is_motor_on == is_running {
            return;
        }
        
        if is_running {
            self.motor_start_time_us = master_clock;
        } else {
            self.update_and_get_angle(master_clock);
        }
        
        self.is_motor_on = is_running;
        
        // :? Is the motor running,
        if is_running && self.stream_length > 1 {
            // Motor turned on, start DMA!
            Self::start_floppy_dma();
        } else {
            // Motor turned off / no stream, stop DMA!
            Self::stop_floppy_dma();
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_speed_zone(&mut self, expected_speed_zone: u8) {
        self.current_speed_zone = expected_speed_zone & 3;
        let register_value = PIO_SPEED_ZONES[self.current_speed_zone as usize];
        let clk_int = (register_value >> 8) as u16;
        let clk_frac = (register_value & 0xFF) as u8;
        let pio = pac::PIO0;
        pio.sm(0).clkdiv().write(|w| {
            w.set_int(clk_int);
            w.set_frac(clk_frac);
        });
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_stream_source(&mut self, stream: &'static TrackBuffer, master_clock: u32) {
        // :? Is this a data holding stream?
        let stream_length = stream.length as u32;
        let track_offset = match stream.layout {
            TrackLayout::Warped(_) => 0,
            TrackLayout::Void => 0,
            TrackLayout::Accurate => {
                let current_angle_us = self.update_and_get_angle(master_clock);
                (current_angle_us * stream_length) / 200_000
            }
        };

        // Set the stream source with an offset
        self.set_stream_source_offset(stream, track_offset);

        // Set the new stream length
        self.stream_length = stream_length;
    }

}
