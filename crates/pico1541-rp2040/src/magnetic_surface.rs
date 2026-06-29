use pico1541_common::magnetic_surface::MagneticSurface;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;



#[repr(align(8))]
struct DmaReloadData([u32; 2]);

static mut DMA_RELOAD_DATA: DmaReloadData = DmaReloadData([0, 0]);

pub struct PioMagneticSurface {
    track_handle: EmulatorHandle,
    last_active_index: bool,
}

impl PioMagneticSurface {
    pub fn new(
        mut pio: embassy_rp::pio::Pio<'static, embassy_rp::peripherals::PIO0>,
        track_handle: EmulatorHandle,
        sync_pin: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_10>,
    ) -> Self {
        let prg = crate::pio::magnetic_surface_program();
        let loaded_prg = pio.common.load_program(&prg);
        
        let mut sm = pio.sm0;
        let mut cfg = embassy_rp::pio::Config::default();
        cfg.clock_divider = 31u8.into(); // 125 MHz / 31 = ~4.03 MHz (4 cycles = 1us per bit)
        cfg.use_program(&loaded_prg, &[]);
        
        cfg.shift_in.auto_fill = true;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = true;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.clock_divider = (1_u8).into();

        let mut sync_pio_pin = pio.common.make_pio_pin(sync_pin);
        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sync_pio_pin]);
        cfg.set_set_pins(&[&sync_pio_pin]);
        cfg.set_out_pins(&[&sync_pio_pin]);

        sm.set_config(&cfg);
        sm.set_enable(true);

        let dma = pac::DMA;
        let ch_a = dma.ch(0);

        // Channel A config (Worker 1)
        let pio_pac = pac::PIO0;
        ch_a.read_addr().write_value(0); // Set in start_reading
        ch_a.write_addr().write_value(pio_pac.txf(0).as_ptr() as u32);
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_a.set_bswap(true);
        ctrl_a.set_incr_read(true);
        ctrl_a.set_incr_write(false);
        ctrl_a.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_a.set_chain_to(1); // Chain to Channel B
        ctrl_a.set_ring_size(13); // 2^13 = 8192 bytes
        ctrl_a.set_ring_sel(false); // Ring on read address
        ctrl_a.set_en(false);
        ch_a.ctrl_trig().write_value(ctrl_a);

        // Channel B config (Worker 2)
        let ch_b = dma.ch(1);
        ch_b.read_addr().write_value(0); // Set in start_reading
        ch_b.write_addr().write_value(pio_pac.txf(0).as_ptr() as u32);

        let mut ctrl_b = ch_b.ctrl_trig().read();
        ctrl_b.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_b.set_bswap(true);
        ctrl_b.set_incr_read(true);
        ctrl_b.set_incr_write(false);
        ctrl_b.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_b.set_chain_to(0); // Chain back to Channel A
        ctrl_b.set_ring_size(13); // 2^13 = 8192 bytes
        ctrl_b.set_ring_sel(false); // Ring on read address
        ctrl_b.set_en(false);
        ch_b.ctrl_trig().write_value(ctrl_b);

        Self {
            track_handle,
            last_active_index: false,
        }
    }
}

impl MagneticSurface for PioMagneticSurface {
    fn is_sync_active(&self) -> bool {
        false
    }

    fn try_read_byte(&mut self) -> Option<u32> {
        // Automatically switch DMA to new buffer when track finishes loading!
        let current_active_index = self.track_handle.get_active_index();
        if current_active_index != self.last_active_index {
            self.last_active_index = current_active_index;
            let track_id = self.track_handle.get_active_track_id();
            self.start_reading(track_id, 0);
        }

        let pio = pac::PIO0;
        
        let dma_busy = pac::DMA.ch(0).ctrl_trig().read().busy();
        let trans_count = pac::DMA.ch(0).trans_count().read();
        let read_addr = pac::DMA.ch(0).read_addr().read();
        let fstat = pio.fstat().read();
        
        // AGGRESSIVE DEBUG LOGGING
        static mut DEBUG_TICK_COUNT: u32 = 0;
        unsafe {
            DEBUG_TICK_COUNT += 1;
            if DEBUG_TICK_COUNT % 100000 == 0 {
                let pc = pio.sm(0).addr().read();
                defmt::info!("PIO STATE - PC: {}, TXF Level: {}, RXF empty: {}, DMA Busy: {}, DMA Count: {}, DMA Addr: {:X}", 
                    pc.addr(), pio.flevel().read().tx0(), fstat.rxempty() & 1, dma_busy, trans_count, read_addr);
            }
        }

        if (fstat.rxempty() & 1) == 0 {
            let val = pio.rxf(0).read();
            
            // DEBUG LOGGING
            static mut DEBUG_BYTE_COUNT: u32 = 0;
            unsafe {
                if DEBUG_BYTE_COUNT < 32 {
                    if val == 0xFFFFFFFF {
                        defmt::info!("PIO DEBUG: Received SYNC token");
                    } else {
                        defmt::info!("PIO DEBUG: Received Data = {:02X}", val);
                    }
                    DEBUG_BYTE_COUNT += 1;
                }
            }
            
            Some(val)
        } else {
            None
        }
    }

    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        let dma = pac::DMA;
        let ch_a = dma.ch(0);
        let remaining = ch_a.trans_count().read();
        
        let track_length = self.track_handle.track_byte_size as u32;
        if track_length >= remaining {
            track_length - remaining
        } else {
            0
        }
    }

    fn write_byte_at(&mut self, index: u32, data: u8) {
        self.track_handle.seek_to_byte(index);
        self.track_handle.next_write_byte(0, 0xFF, data);
    }
    
    fn start_reading(&mut self, track_id: u8, _rotation_fixed: u32) {
        self.track_handle.update_head_half_track(track_id);
        
        // Ensure track is loaded and padded. We use the full 8192-byte underlying buffer pointer.
        let track_ptr = self.track_handle.get_full_buffer_ptr();

        let dma = pac::DMA;
        
        // Abort any ongoing transfers cleanly
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_en(false);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
        
        let mut ctrl_b = dma.ch(1).ctrl_trig().read();
        ctrl_b.set_en(false);
        dma.ch(1).ctrl_trig().write_value(ctrl_b);
        
        let full_track = track_id / 2;
        // 1541 uses 4 different bitrates depending on the track zone.
        // The PIO bit loop takes 4 clock cycles per bit.
        // We set the PIO clock such that 4 cycles equal the exact bitrate duration.
        let (clk_int, clk_frac) = match full_track {
            1..=17 => (101, 144), // Zone 1: 307.692 kbps
            18..=24 => (109, 96),  // Zone 2: 285.714 kbps
            25..=30 => (117, 48),  // Zone 3: 266.666 kbps
            _ => (125, 0),         // Zone 4: 250.000 kbps
        };

        // Clear the PIO RX and TX FIFOs
        let pio = pac::PIO0;
        
        pio.sm(0).clkdiv().write(|w| {
            w.set_int(clk_int);
            w.set_frac(clk_frac);
        });

        pio.ctrl().write(|w| w.set_sm_restart(1)); // Restart SM to reset its internal state (Y, ISR, OSR)
        pio.ctrl().write(|w| w.set_sm_enable(1));
        
        // Drain any lingering data in RX FIFO
        while (pio.fstat().read().rxempty() & 1) == 0 {
            let _ = pio.rxf(0).read();
        }
        
        let word_count = 8192 / 4; // Transfer entire 8192 bytes (2048 words) per cycle

        // Setup Channel B (Worker 2)
        dma.ch(1).read_addr().write_value(track_ptr as u32);
        dma.ch(1).trans_count().write_value(word_count);
        
        // Re-enable Channel B WITHOUT triggering (it will wait for Channel A to chain to it)
        ctrl_b.set_en(true);
        // SAFETY: writing to al1_ctrl sets CTRL but does not trigger
        unsafe { dma.ch(1).al1_ctrl().write_value(core::mem::transmute(ctrl_b)) };

        // Setup Worker (Channel A)
        dma.ch(0).read_addr().write_value(track_ptr as u32);
        dma.ch(0).trans_count().write_value(word_count);
        
        // Re-enable and TRIGGER Worker A
        ctrl_a.set_en(true);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
    }
    
    fn stop_reading(&mut self) {
        let dma = pac::DMA;
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_en(false);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);

        let mut ctrl_b = dma.ch(1).ctrl_trig().read();
        ctrl_b.set_en(false);
        dma.ch(1).ctrl_trig().write_value(ctrl_b);
    }

    fn is_write_protected(&self) -> bool {
        self.track_handle.is_write_protected()
    }

    fn pop_force_disk_change(&mut self) -> bool {
        self.track_handle.pop_force_disk_change()
    }
}
