use pico1541_common::magnetic_surface::MagneticSurface;

use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;


/// Cached raw register addresses to avoid PAC proxy construction on every APB access.
/// On RP2040, the PAC types are zero-sized, but the address computation for indexed
/// registers (ch(0), sm(0), txf(0), rxf(0)) involves multiplication that may not be
/// fully elided by the compiler at `-Os`.
struct CachedRegs {
    /// PIO0 FSTAT register address
    pio0_fstat: *const u32,
    /// PIO0 RXF(0) register address
    pio0_rxf0: *const u32,
    /// DMA CH0 READ_ADDR register address
    dma_ch0_read_addr: *mut u32,
    /// DMA CH0 TRANS_COUNT register address
    dma_ch0_trans_count: *mut u32,
    /// DMA CH0 CTRL_TRIG register address (for read-modify-write to re-enable)
    dma_ch0_ctrl_trig: *mut u32,
}

impl CachedRegs {
    fn new() -> Self {
        let pio = pac::PIO0;
        let dma = pac::DMA;
        Self {
            pio0_fstat: pio.fstat().as_ptr() as *const u32,
            pio0_rxf0: pio.rxf(0).as_ptr() as *const u32,
            dma_ch0_read_addr: dma.ch(0).read_addr().as_ptr() as *mut u32,
            dma_ch0_trans_count: dma.ch(0).trans_count().as_ptr() as *mut u32,
            dma_ch0_ctrl_trig: dma.ch(0).ctrl_trig().as_ptr() as *mut u32,
        }
    }
}


pub struct PioMagneticSurface {
    track_handle: EmulatorHandle,
    last_active_index: bool,
    sub_cycle_counter: u32,
    cached_regs: CachedRegs,
    /// Cached ch0 CTRL_TRIG value with EN=true, for fast restart
    cached_ctrl_enabled: u32,
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
        cfg.clock_divider = 162u8.into(); // Will be overridden by set_speed_zone
        cfg.shift_in.auto_fill = false;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false; // Handled manually in PIO
        cfg.shift_out.threshold = 32; 
        cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Left;

        cfg.use_program(&loaded_prg, &[]);

        sm.set_config(&cfg);
        sm.set_enable(true);

        let dma_pac = pac::DMA;
        let ch_a = dma_pac.ch(0);

        // Channel A config (Worker 1)
        let pio_pac = pac::PIO0;
        ch_a.read_addr().write_value(0); // Set in start_reading
        ch_a.write_addr().write_value(pio_pac.txf(0).as_ptr() as u32); // Write to TXF offset 0
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_data_size(pac::dma::vals::DataSize::SIZE_BYTE); // 8-bit transfers to match exact track length
        ctrl_a.set_bswap(false);
        ctrl_a.set_incr_read(true);
        ctrl_a.set_incr_write(false);
        ctrl_a.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_a.set_chain_to(0); // Chain to itself (won't trigger loop since trans_count will be 0)
        ctrl_a.set_ring_size(0); // No ring, we manually restart at exact track length boundaries
        ctrl_a.set_ring_sel(false);
        ctrl_a.set_en(false);
        ch_a.ctrl_trig().write_value(ctrl_a);

        // Cache the enabled control value for fast restart.
        // Write it with EN=true, then read back the raw register value.
        ctrl_a.set_en(true);
        ch_a.ctrl_trig().write_value(ctrl_a);
        let cached_ctrl_enabled = unsafe { core::ptr::read_volatile(dma_pac.ch(0).ctrl_trig().as_ptr() as *const u32) };
        // Immediately disable again — start_reading will enable when ready
        ch_a.ctrl_trig().modify(|w| w.set_en(false));

        Self {
            track_handle,
            last_active_index: false,
            sub_cycle_counter: 0,
            cached_regs: CachedRegs::new(),
            cached_ctrl_enabled,
        }
    }
}

impl PioMagneticSurface {
    fn abort_dma_ch0() {
        let dma = pac::DMA;
        dma.ch(0).ctrl_trig().modify(|w| w.set_en(false));
        dma.chan_abort().modify(|w| w.set_chan_abort(1 << 0));
        while dma.chan_abort().read().chan_abort() & (1 << 0) != 0 {}
    }

    fn clear_pio_fifos() {
        let pio = pac::PIO0;
        let shiftctrl = pio.sm(0).shiftctrl();
        shiftctrl.modify(|w| w.set_fjoin_rx(!w.fjoin_rx()));
        shiftctrl.modify(|w| w.set_fjoin_rx(!w.fjoin_rx()));
        
        // Also restart state machine to clear internal shifting state (OSR/ISR)
        pio.ctrl().write(|w| {
            w.set_sm_restart(1 << 0);
            w.set_sm_enable(1 << 0);
        });
    }
}

impl MagneticSurface for PioMagneticSurface {

    fn try_read_byte(&mut self, _delta_cycles: u32) -> Option<u32> {
        self.sub_cycle_counter = self.sub_cycle_counter.wrapping_add(_delta_cycles);
        
        // Throttle APB checks to once every 16 cycles (16 us)
        // This is safe because PIO TX FIFO holds 4 bytes (100+ us buffer)
        // and RX FIFO won't overflow in 16us.
        if self.sub_cycle_counter >= 16 {
            self.sub_cycle_counter = 0;

            let current_active_index = self.track_handle.get_active_index();
            if current_active_index != self.last_active_index {
                self.last_active_index = current_active_index;
                let track_id = self.track_handle.get_active_track_id();
                self.start_reading(track_id, 0);
            }

            // Check if DMA ch0 is done (busy flag) and restart for track wrapping.
            // Uses cached register pointers to avoid PAC proxy construction overhead.
            let ctrl_trig = unsafe { core::ptr::read_volatile(self.cached_regs.dma_ch0_ctrl_trig) };
            if ctrl_trig & (1 << 24) == 0 { // bit 24 = BUSY
                let track_ptr = self.track_handle.get_full_buffer_ptr();
                let track_length = self.track_handle.get_active_track_length() as u32;
                if track_length > 0 {
                    unsafe {
                        core::ptr::write_volatile(self.cached_regs.dma_ch0_read_addr, track_ptr as u32);
                        core::ptr::write_volatile(self.cached_regs.dma_ch0_trans_count, track_length);
                        core::ptr::write_volatile(self.cached_regs.dma_ch0_ctrl_trig, self.cached_ctrl_enabled);
                    }
                }
            }
        }

        // Read PIO RX FIFO using cached register pointers.
        // fstat bit 0 (RXEMPTY for SM0): 0 = data available, 1 = empty
        // The RXEMPTY field is at bits [11:8] in fstat, bit 8 = SM0
        let fstat = unsafe { core::ptr::read_volatile(self.cached_regs.pio0_fstat) };
        
        if (fstat & (1 << 8)) == 0 { // bit 8 = RXEMPTY for SM0
            let val = unsafe { core::ptr::read_volatile(self.cached_regs.pio0_rxf0) };
            if val > 0xFF {
                Some(0xFFFFFFFF) // SYNC detected
            } else {
                Some(val)
            }
        } else {
            None
        }
    }

    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        let remaining = unsafe { core::ptr::read_volatile(self.cached_regs.dma_ch0_trans_count as *const u32) };
        
        let track_length = self.track_handle.get_active_track_length() as u32;
        track_length.saturating_sub(remaining)
    }

    fn write_byte_at(&mut self, index: u32, data: u8) {
        self.track_handle.seek_to_byte(index);
        self.track_handle.next_write_byte(0, 0xFF, data);
    }
    
    fn start_reading(&mut self, track_id: u8, _rotation_fixed: u32) {
        self.track_handle.update_head_half_track(track_id);
        
        // Ensure track is loaded and padded. We use the full 8192-byte underlying buffer pointer.
        let track_ptr = self.track_handle.get_full_buffer_ptr();

        Self::abort_dma_ch0();
        
        let dma = pac::DMA;
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_incr_read(true); // Restore incr_read in case it was disabled by noise injection
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
        
        // Update cached ctrl value in case incr_read was toggled by noise injection
        ctrl_a.set_en(true);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
        self.cached_ctrl_enabled = unsafe { core::ptr::read_volatile(self.cached_regs.dma_ch0_ctrl_trig as *const u32) };
        dma.ch(0).ctrl_trig().modify(|w| w.set_en(false));
        
        Self::clear_pio_fifos();
        
        // Setup Worker (Channel A) with the exact byte count!
        // try_read_byte will poll the busy flag and reload it instantly when it completes.
        let track_length = self.track_handle.get_active_track_length() as u32;
        dma.ch(0).read_addr().write_value(track_ptr as u32);
        dma.ch(0).trans_count().write_value(track_length);
        
        // Re-enable and TRIGGER Worker A
        if track_length > 0 {
            unsafe {
                core::ptr::write_volatile(self.cached_regs.dma_ch0_ctrl_trig, self.cached_ctrl_enabled);
            }
        }
    }
    
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
        // PIO reads 1 bit every 5 cycles.
        // Zone 3: 26us/byte = 3.25us/bit.
        // Zone 2: 28us/byte = 3.5us/bit.
        // Zone 1: 30us/byte = 3.75us/bit.
        // Zone 0: 32us/byte = 4.0us/bit.
        let multiplier = match zone {
            3 => 325,
            2 => 350,
            1 => 375,
            _ => 400,
        };
        let div_256 = (pico1541_common::SYSTEM_CLOCK_FREQ as u64 * multiplier * 256) / 500_000_000;
        let clk_int = (div_256 / 256) as u16;
        let clk_frac = (div_256 % 256) as u8;

        let pio = pac::PIO0;
        pio.sm(0).clkdiv().write(|w| {
            w.set_int(clk_int);
            w.set_frac(clk_frac);
        });
    }

    fn inject_analog_noise(&mut self) {
        static NOISE_BUFFER: [u8; 1] = [0x22];

        Self::abort_dma_ch0();
        Self::clear_pio_fifos();

        let dma = pac::DMA;
        // Set up DMA to continuously feed the noise byte
        dma.ch(0).read_addr().write_value(NOISE_BUFFER.as_ptr() as u32);
        dma.ch(0).trans_count().write_value(0xFFFFFFFF);
        
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_incr_read(false); // Do not increment read address, keep sending 0x22
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
