use crate::track_buffer::EmulatorHandle;

pub trait MagneticSurface {
    fn is_sync_active(&self) -> bool;
    fn try_read_byte(&mut self) -> Option<u8>;
    fn get_absolute_head_position(&self, master_clock: u32) -> u32;
    fn write_byte_at(&mut self, index: u32, data: u8);
    fn start_reading(&mut self, track_id: u8, rotation_fixed: u32);
    fn stop_reading(&mut self);
    fn is_write_protected(&self) -> bool;
    fn pop_force_disk_change(&mut self) -> bool;
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
        let expected_track_id = self.track_handle.get_head_half_track();
        let gcr_byte = self.track_handle.next_read_byte(expected_track_id, 0xFF, true);
        let next_gcr_byte = self.track_handle.peek_next_byte();
        
        // SYNC is 0xFF. If we see two in a row, we are in SYNC.
        if gcr_byte == 0xFF && next_gcr_byte == 0xFF {
            self.is_sync = true;
        } else if gcr_byte != 0xFF && self.is_sync {
            // End of SYNC mark.
            self.is_sync = false;
        }
        
        Some(gcr_byte)
    }

    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        self.track_handle.get_current_byte_idx() as u32
    }

    fn write_byte_at(&mut self, index: u32, data: u8) {
        self.track_handle.seek_to_byte(index);
        let expected_track_id = self.track_handle.get_head_half_track();
        self.track_handle.next_write_byte(expected_track_id, 0xFF, data);
    }

    fn start_reading(&mut self, track_id: u8, _rotation_fixed: u32) {
        self.track_handle.update_head_half_track(track_id);
    }

    fn stop_reading(&mut self) {}

    fn is_write_protected(&self) -> bool {
        self.track_handle.is_write_protected()
    }

    fn pop_force_disk_change(&mut self) -> bool {
        self.track_handle.pop_force_disk_change()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track_buffer::DualCoreTrackBuffer;

    #[test]
    fn test_is_sync_logic() {
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new()));
        let (manager, emulator) = dual_buffer.split();

        let active_buf = manager.start_track_modification();
        active_buf[0] = 0xFF;
        active_buf[1] = 0xFF;
        active_buf[2] = 0xFF;
        active_buf[3] = 0x55;
        active_buf[4] = 0xFF;
        active_buf[5] = 0x55;
        manager.commit_loaded_track(0, crate::track_buffer::TrackLayout::Accurate, 6);

        let mut surface = SoftwareMagneticSurface::new(emulator);
        
        surface.start_reading(0, 0);
        
        let b0 = surface.try_read_byte().unwrap();
        assert_eq!(b0, 0xFF);
        assert_eq!(surface.is_sync_active(), true);

        let b1 = surface.try_read_byte().unwrap();
        assert_eq!(b1, 0xFF);
        assert_eq!(surface.is_sync_active(), true);

        let b2 = surface.try_read_byte().unwrap();
        assert_eq!(b2, 0xFF);
        assert_eq!(surface.is_sync_active(), true);

        let b3 = surface.try_read_byte().unwrap();
        assert_eq!(b3, 0x55);
        assert_eq!(surface.is_sync_active(), false);

        let b4 = surface.try_read_byte().unwrap();
        assert_eq!(b4, 0xFF);
        assert_eq!(surface.is_sync_active(), false);
        
        let b5 = surface.try_read_byte().unwrap();
        assert_eq!(b5, 0x55);
        assert_eq!(surface.is_sync_active(), false);
    }
}
