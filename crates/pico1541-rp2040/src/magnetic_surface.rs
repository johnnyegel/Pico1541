use pico1541_common::magnetic_surface::MagneticSurface;
use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;


// Speed zone PIO timing
// The PIO program executes exactly 8 cycles per *bit*, not per byte!
const PIO_CYCLES_PER_BIT: u64 = 8; 
// We use exactly 100M for perfect real-time emulation.
const PIO_TIMING_DENOMINATOR: u64 = 100_000_000 * PIO_CYCLES_PER_BIT;

// This const function executes entirely at compile time.
const fn calc_speed_zone_register(multiplier: u64) -> u32 {
    // 1. Calculate the scaled fraction: (Clock * Time * 256) / Denominator
    // 64-bit math guarantees no overflow during the massive multiplication step.
    let div_256 = (pico1541_common::SYSTEM_CLOCK_FREQ as u64 * multiplier * 256) / PIO_TIMING_DENOMINATOR;
    
    // Cast back to u32 for the hardware registers safely
    div_256 as u32
}

const PIO_SPEED_ZONES: [u32; 4] = [
    calc_speed_zone_register(400), // Zone 0: 4.00 µs
    calc_speed_zone_register(375), // Zone 1: 3.75 µs
    calc_speed_zone_register(350), // Zone 2: 3.50 µs
    calc_speed_zone_register(325), // Zone 3: 3.25 µs
];

// Static buffers for DMA
static mut READ_BUFFER: u16 = 0x8000;
static mut CURRENT_TRACK_PTR: u32 = 0;

pub struct PioMagneticSurface {
    track_handle: EmulatorHandle,
    last_active_index: bool,

    _sync_pin: embassy_rp::pio::Pin<'static, embassy_rp::peripherals::PIO0>,
    _loaded_prg: embassy_rp::pio::LoadedProgram<'static, embassy_rp::peripherals::PIO0>,
    _sm: embassy_rp::pio::StateMachine<'static, embassy_rp::peripherals::PIO0, 0>,
}

impl PioMagneticSurface {
    pub fn new(
        mut pio: embassy_rp::pio::Pio<'static, embassy_rp::peripherals::PIO0>,
        track_handle: EmulatorHandle,
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
        // SIO will be used to read this pin state directly
        let sync_pin = pio.common.make_pio_pin(unsafe { embassy_rp::peripherals::PIN_10::steal() });
        cfg.set_set_pins(&[&sync_pin]);

        cfg.use_program(&loaded_prg, &[]);

        sm.set_config(&cfg);
        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sync_pin]);
        sm.set_enable(true);

        Self::init_1541_dma_channels();

        Self {
            last_active_index: !track_handle.get_active_index(), // Force initial start_reading
            track_handle,
            _sync_pin: sync_pin,
            _loaded_prg: loaded_prg,
            _sm: sm,
        }
    }

    fn init_1541_dma_channels() {
        let dma = pac::DMA;
        let pio = pac::PIO0;

        let ch_a = dma.ch(0);
        let ch_c = dma.ch(2);

        // --- Channel A: TX Worker (Streams data to PIO) ---
        // Paced by PIO TX DREQ, 32-bit words, read inc, write no inc. Chains to B.
        ch_a.read_addr().write_value(0); // Set later
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
        ctrl_b.set_en(true);
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
        // Read physical state of GPIO 10 via SIO
        (sio.gpio_in(0).read() & (1 << 10)) != 0
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

    #[link_section = ".data.ram_func"]
    fn abort_dma_ch0() {
        let dma = pac::DMA;
        dma.ch(0).ctrl_trig().modify(|w| w.set_en(false));
        dma.chan_abort().modify(|w| w.set_chan_abort(1 << 0));
        while dma.chan_abort().read().chan_abort() & (1 << 0) != 0 {}
    }

    #[link_section = ".data.ram_func"]
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
        let current_active_index = self.track_handle.get_active_index();
        if current_active_index != self.last_active_index {
            self.last_active_index = current_active_index;
            let track_id = self.track_handle.get_active_track_id();
            self.start_reading(track_id, 0);
        }

        self.try_read_ram_byte()
    }

    #[link_section = ".data.ram_func"]
    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        let dma = pac::DMA;
        let remaining = dma.ch(0).trans_count().read();
        let track_length = self.track_handle.get_active_track_length() as u32;
        track_length.saturating_sub(remaining)
    }

    #[link_section = ".data.ram_func"]
    fn write_byte_at(&mut self, index: u32, data: u8) {
        self.track_handle.seek_to_byte(index);
        self.track_handle.next_write_byte(0, 0xFF, data);
    }
    
    #[link_section = ".data.ram_func"]
    fn start_reading(&mut self, track_id: u8, _rotation_fixed: u32) {
        self.track_handle.update_head_half_track(track_id);
        
        let track_ptr = self.track_handle.get_full_buffer_ptr() as u32;
        let track_length = self.track_handle.get_active_track_length() as u32;

        Self::abort_dma_ch0();
        
        unsafe {
            CURRENT_TRACK_PTR = track_ptr;
            core::ptr::write_volatile(core::ptr::addr_of_mut!(READ_BUFFER), 0x8000);
        }

        let dma = pac::DMA;

        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_incr_read(true);
        ctrl_a.set_ring_size(0); // Disable wrapping (was enabled by inject_analog_noise)
        ctrl_a.set_chain_to(1); // Restore chaining to Channel B
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
        
        Self::clear_pio_fifos();
        
        dma.ch(0).read_addr().write_value(track_ptr);
        dma.ch(0).trans_count().write_value(track_length);
        
        if track_length > 0 {
            ctrl_a.set_en(true);
            dma.ch(0).ctrl_trig().write_value(ctrl_a);
        }
    }
    
    #[link_section = ".data.ram_func"]
    fn stop_reading(&mut self) {
        Self::abort_dma_ch0();
    }

    fn is_write_protected(&self) -> bool {
        self.track_handle.is_write_protected()
    }

    fn pop_force_disk_change(&mut self) -> bool {
        self.track_handle.pop_force_disk_change()
    }

    fn set_speed_zone(&mut self, zone: u8) {
        // Bitwise AND with 3 safely bounds the zone to [0, 1, 2, 3]
        let register_value = PIO_SPEED_ZONES[(zone & 3) as usize];
        
        // Slice the pre-calculated fixed-point value into Int and Frac
        let clk_int = (register_value >> 8) as u16;
        let clk_frac = (register_value & 0xFF) as u8;
    
        let pio = pac::PIO0;
        pio.sm(0).clkdiv().write(|w| {
            w.set_int(clk_int);
            w.set_frac(clk_frac);
        });
    }

    fn inject_analog_noise(&mut self) {
        #[repr(align(16))]
        struct NoiseBuffer([u8; 16]);
        
        static NOISE_BUFFER: NoiseBuffer = NoiseBuffer([
            0x55, 0xAA, 0x33, 0xCC, 0x0F, 0xF0, 0x55, 0xAA,
            0x33, 0xCC, 0x0F, 0xF0, 0x55, 0xAA, 0x33, 0xCC
        ]);
        
        Self::abort_dma_ch0();
        Self::clear_pio_fifos();

        unsafe {
            CURRENT_TRACK_PTR = NOISE_BUFFER.0.as_ptr() as u32;
        }

        let dma = pac::DMA;
        dma.ch(0).read_addr().write_value(NOISE_BUFFER.0.as_ptr() as u32);
        dma.ch(0).trans_count().write_value(0xFFFFFFFF);
        
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_incr_read(false);
        ctrl_a.set_en(true);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
    }

    fn get_active_speed_zone(&self) -> Option<u8> {
        self.track_handle.get_active_speed_zone()
    }

    fn get_active_track_length(&self) -> usize {
        self.track_handle.get_active_track_length()
    }
}
