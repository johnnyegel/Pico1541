use pico1541_common::magnetic_surface::MagneticSurface;
use pico1541_common::track_buffer::TrackStream;
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
static mut READ_BUFFER: u16 = 0x8000;
static mut CURRENT_TRACK_PTR: u32 = 0;

pub struct PioMagneticSurface {
    is_motor_on: bool,
    stream_length: u32,
    last_head_pos: u32,

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
            last_head_pos: 0,
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
        let ch_c = dma.ch(2);

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

        // --- Channel C: RX Sentinel Ring (Extracts data to RAM) ---
        // Paced by PIO RX DREQ, 16-bit, read no inc, write inc, ring size 5.
        ch_c.read_addr().write_value(pio.rxf(0).as_ptr() as u32);
        ch_c.write_addr().write_value(core::ptr::addr_of!(READ_BUFFER) as u32);
        ch_c.trans_count().write_value(0xFFFFFFFF); // Infinite
        
        let mut ctrl_c = ch_c.ctrl_trig().read();
        ctrl_c.set_data_size(pac::dma::vals::DataSize::SIZE_HALFWORD);
        ctrl_c.set_incr_read(false);
        ctrl_c.set_incr_write(false);
        ctrl_c.set_treq_sel(pac::dma::vals::TreqSel::PIO0_RX0);
        ctrl_c.set_ring_sel(false); 
        ctrl_c.set_ring_size(0);
        ctrl_c.set_chain_to(2);
        ctrl_c.set_en(true);
        ch_c.ctrl_trig().write_value(ctrl_c);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn is_sync_active() -> bool {
        let sio = pac::SIO;
        // Read physical state of SYNC GPIO via SIO
        (sio.gpio_in(0).read() & (1 << GPIO_SYNC_PIN)) != 0
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn try_read_ram_byte(&mut self) -> Option<u8> {
        unsafe {
            let ptr = core::ptr::addr_of_mut!(READ_BUFFER);
            let val = core::ptr::read_volatile(ptr);
            if (val & 0x8000) == 0 {
                let byte = (val & 0xFF) as u8;
                core::ptr::write_volatile(ptr, 0x8000);
                Some(byte)
            } else {
                None
            }
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
    fn set_stream_source_offset(&self, stream: TrackStream, offset: u32) {
        // :: Ensure the floppy DMA channels are stopped and the PIO FIFOs are cleared.
        Self::stop_floppy_dma();
        Self::clear_pio_fifos();

        // :: Set the new track pointer and length for the DMA transfer    
        let track_ptr = stream.as_dma_ptr();
        let track_length = stream.length;

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
    fn set_motor_running(&mut self, is_running: bool) {
        if self.is_motor_on == is_running {
            return;
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
    fn set_stream_source(&mut self, stream: TrackStream) {
        // :? Is this a data holding stream?
        // FIXME! BAD HACK: Checking the length to determine if this is a noise track.
        let track_offset = if stream.length <= 16 {
            // -> No, then record where the read head is at.
            self.last_head_pos = self.get_absolute_head_position(0);
            // No offset needed
            0
        }
        else if stream.length != self.stream_length {
            // -> Yes, then we realign the head by scaling the last head position
            (self.last_head_pos * stream.length) / self.stream_length
        }
        else {
            self.last_head_pos
        };

        // Set the stream source with an offset
        self.set_stream_source_offset(stream, track_offset);

        // Set the new stream length
        self.stream_length = stream.length;
    }

}
