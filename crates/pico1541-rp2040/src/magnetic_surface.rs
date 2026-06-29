use pico1541_common::magnetic_surface::MagneticSurface;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;

static mut DMA_RELOAD_DATA: [u32; 2] = [0, 0];

pub struct PioMagneticSurface {
    track_handle: EmulatorHandle,
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
        cfg.use_program(&loaded_prg, &[]);
        
        cfg.shift_in.auto_fill = true;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = true;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Left;
        
        cfg.clock_divider = (1_u8).into();

        let sync_pio_pin = pio.common.make_pio_pin(sync_pin);
        cfg.set_set_pins(&[&sync_pio_pin]);

        sm.set_config(&cfg);
        sm.set_enable(true);

        let dma = pac::DMA;
        let ch_a = dma.ch(0);

        // Channel A config (Worker)
        let pio_pac = pac::PIO0;
        ch_a.read_addr().write_value(0);
        ch_a.write_addr().write_value(pio_pac.txf(0).as_ptr() as u32);
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_a.set_bswap(true);
        ctrl_a.set_incr_read(true);
        ctrl_a.set_incr_write(false);
        ctrl_a.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_a.set_chain_to(1); // Chain to Channel 1 (Reloader)
        ctrl_a.set_en(false);
        ch_a.ctrl_trig().write_value(ctrl_a);

        // Channel B config (Reloader)
        let ch_b = dma.ch(1);
        ch_b.read_addr().write_value(unsafe { DMA_RELOAD_DATA.as_ptr() as u32 });
        // Write to Channel A's AL3_TRANS_COUNT (which is AL3_CTRL offset + 0x08).
        // It writes 2 words: TRANS_COUNT and READ_ADDR_TRIG.
        ch_b.write_addr().write_value(dma.ch(0).al3_ctrl().as_ptr() as u32 + 8);

        let mut ctrl_b = ch_b.ctrl_trig().read();
        ctrl_b.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_b.set_bswap(false);
        ctrl_b.set_incr_read(true); // Reads 2 words from RELOAD_DATA
        ctrl_b.set_incr_write(true); // Writes 2 words to AL3 (TRANS_COUNT then READ_ADDR_TRIG)
        ctrl_b.set_treq_sel(pac::dma::vals::TreqSel::PERMANENT); // Always ready
        ctrl_b.set_chain_to(1); // Do not chain (wait for trigger from A)
        ctrl_b.set_ring_size(3); // 2^3 = 8 bytes = 2 words ring buffer on read!
        ctrl_b.set_ring_sel(false); // Ring on read address
        ctrl_b.set_en(true); // Keep enabled
        ch_b.ctrl_trig().write_value(ctrl_b);

        Self {
            track_handle,
        }
    }
}

impl MagneticSurface for PioMagneticSurface {
    fn is_sync_active(&self) -> bool {
        false
    }

    fn try_read_byte(&mut self) -> Option<u32> {
        let pio = pac::PIO0;
        
        let fstat = pio.fstat().read();
        if (fstat.rxempty() & 1) == 0 {
            Some(pio.rxf(0).read())
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
        let data = self.track_handle.get_active_buffer_slice();

        let dma = pac::DMA;
        
        // Disable both channels to cleanly restart
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_en(false);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
        
        let mut ctrl_b = dma.ch(1).ctrl_trig().read();
        ctrl_b.set_en(false);
        dma.ch(1).ctrl_trig().write_value(ctrl_b);
        
        unsafe {
            DMA_RELOAD_DATA[0] = ((data.len() + 3) / 4) as u32; // SIZE_WORD transfers
            DMA_RELOAD_DATA[1] = data.as_ptr() as u32;
        }

        // Restart Reloader (Channel B)
        dma.ch(1).read_addr().write_value(unsafe { DMA_RELOAD_DATA.as_ptr() as u32 });
        dma.ch(1).trans_count().write_value(2);
        ctrl_b.set_en(true);
        dma.ch(1).ctrl_trig().write_value(ctrl_b);

        // Manually trigger the Reloader once to start the infinite ping-pong!
        dma.multi_chan_trigger().write(|w| w.set_multi_chan_trigger(1 << 1));
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
