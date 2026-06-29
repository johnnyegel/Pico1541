use pico1541_common::magnetic_surface::MagneticSurface;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::pac;
use pico1541_common::track_buffer::EmulatorHandle;



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
        cfg.set_out_pins(&[&sync_pio_pin]);

        sm.set_config(&cfg);
        sm.set_enable(true);

        let dma = pac::DMA;
        let ch_a = dma.ch(0);

        // Channel A config
        let pio_pac = pac::PIO0;
        ch_a.read_addr().write_value(0);
        ch_a.write_addr().write_value(pio_pac.txf(0).as_ptr() as u32);
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
        ctrl_a.set_bswap(true);
        ctrl_a.set_incr_read(true);
        ctrl_a.set_incr_write(false);
        ctrl_a.set_treq_sel(pac::dma::vals::TreqSel::PIO0_TX0);
        ctrl_a.set_en(false);
        ch_a.ctrl_trig().write_value(ctrl_a);

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
        
        if !pac::DMA.ch(0).ctrl_trig().read().busy() {
            let data = self.track_handle.get_active_buffer_slice();
            pac::DMA.ch(0).read_addr().write_value(data.as_ptr() as u32);
            pac::DMA.ch(0).trans_count().write_value(((data.len() + 3) / 4) as u32);
            let mut ctrl = pac::DMA.ch(0).ctrl_trig().read();
            ctrl.set_en(true);
            pac::DMA.ch(0).ctrl_trig().write_value(ctrl);
        }

        let fstat = pio.fstat().read();
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
        let data = self.track_handle.get_active_buffer_slice();

        let dma = pac::DMA;
        let ch_a = dma.ch(0);

        ch_a.read_addr().write_value(data.as_ptr() as u32);
        ch_a.trans_count().write_value(((data.len() + 3) / 4) as u32);
        
        let mut ctrl_a = ch_a.ctrl_trig().read();
        ctrl_a.set_en(true);
        ch_a.ctrl_trig().write_value(ctrl_a);
    }
    
    fn stop_reading(&mut self) {
        let dma = pac::DMA;
        let mut ctrl_a = dma.ch(0).ctrl_trig().read();
        ctrl_a.set_en(false);
        dma.ch(0).ctrl_trig().write_value(ctrl_a);
    }

    fn is_write_protected(&self) -> bool {
        self.track_handle.is_write_protected()
    }

    fn pop_force_disk_change(&mut self) -> bool {
        self.track_handle.pop_force_disk_change()
    }
}
