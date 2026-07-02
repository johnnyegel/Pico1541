use pico1541_common::magnetic_surface::MagneticSurface;

use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;





pub struct PioMagneticSurface {
    track_handle: EmulatorHandle,
    last_active_index: bool,
    sub_cycle_counter: u32,
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

        let dma = pac::DMA;
        let ch_a = dma.ch(0);

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

        Self {
            track_handle,
            last_active_index: false,
            sub_cycle_counter: 0,
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
    fn is_sync_active(&self) -> bool {
        false
    }

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

            let dma = pac::DMA;
            if !dma.ch(0).ctrl_trig().read().busy() {
                let track_ptr = self.track_handle.get_full_buffer_ptr();
                let track_length = self.track_handle.get_active_track_length() as u32;
                if track_length > 0 {
                    dma.ch(0).read_addr().write_value(track_ptr as u32);
                    dma.ch(0).trans_count().write_value(track_length);
                    let mut ctrl = dma.ch(0).ctrl_trig().read();
                    ctrl.set_en(true);
                    dma.ch(0).ctrl_trig().write_value(ctrl);
                }
            }
        }

        let pio = pac::PIO0;
        let fstat = pio.fstat().read();
        
        if (fstat.rxempty() & 1) == 0 {
            let val = pio.rxf(0).read();
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
        let dma = pac::DMA;
        let ch_a = dma.ch(0);
        let remaining = ch_a.trans_count().read();
        
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
        
        Self::clear_pio_fifos();
        
        // Setup Worker (Channel A) with the exact byte count!
        // try_read_byte will poll the busy flag and reload it instantly when it completes.
        let track_length = self.track_handle.get_active_track_length() as u32;
        dma.ch(0).read_addr().write_value(track_ptr as u32);
        dma.ch(0).trans_count().write_value(track_length);
        
        // Re-enable and TRIGGER Worker A
        if track_length > 0 {
            ctrl_a.set_en(true);
            dma.ch(0).ctrl_trig().write_value(ctrl_a);
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
