use crate::track_buffer::EmulatorHandle;

pub trait MagneticSurface {
    fn is_sync_active(&self) -> bool;
    fn try_read_byte(&mut self) -> Option<u8>;
    fn get_absolute_head_position(&self, master_clock: u32) -> u32;
    fn write_byte_at(&mut self, index: u32, data: u8);
    fn start_reading(&mut self, track_id: u8, rotation_percentage: f64);
    fn stop_reading(&mut self);
}

pub struct SoftwareMagneticSurface {
    pub track_handle: EmulatorHandle,
    is_sync: bool,
}

impl SoftwareMagneticSurface {
    pub fn new(track_handle: EmulatorHandle) -> Self {
        Self { track_handle, is_sync: false }
    }
}

impl MagneticSurface for SoftwareMagneticSurface {
    fn is_sync_active(&self) -> bool {
        self.is_sync
    }

    fn try_read_byte(&mut self) -> Option<u8> {
        // Provide a dummy sector for now, as is_listening is the crucial part for bit-shifting.
        // We assume try_read_byte is called when reading is active.
        let gcr_byte = self.track_handle.next_read_byte(0, 0xFF, true);
        let next_gcr_byte = self.track_handle.peek_next_byte();
        
        // SYNC is 0xFF. If we see two in a row, we are in SYNC.
        if gcr_byte == 0xFF && next_gcr_byte == 0xFF {
            self.is_sync = true;
        } else if gcr_byte == 0xFF && !self.is_sync {
            // End of SYNC mark.
            self.is_sync = false;
        }
        
        Some(gcr_byte)
    }

    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        self.track_handle.get_current_byte_idx() as u32
    }

    fn write_byte_at(&mut self, _index: u32, data: u8) {
        self.track_handle.next_write_byte(0, 0xFF, data);
    }

    fn start_reading(&mut self, track_id: u8, _rotation_percentage: f64) {
        self.track_handle.update_head_half_track(track_id);
    }

    fn stop_reading(&mut self) {}
}
