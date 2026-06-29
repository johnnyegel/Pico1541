use crate::track_buffer::EmulatorHandle;

pub trait MagneticSurface {
    fn is_sync_active(&self) -> bool;
    fn try_read_byte(&mut self) -> Option<u32>;
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
    bit_shift_reg: u8,
    bits_in_reg: u8,
    sync_counter: u8,
    pending_bits: u16,
    pending_bit_count: u8,
}

impl SoftwareMagneticSurface {
    pub fn new(track_handle: EmulatorHandle) -> Self {
        Self { 
            track_handle, 
            is_sync: false,
            bit_shift_reg: 0,
            bits_in_reg: 0,
            sync_counter: 0,
            pending_bits: 0,
            pending_bit_count: 0,
        }
    }
}

impl MagneticSurface for SoftwareMagneticSurface {
    fn is_sync_active(&self) -> bool {
        self.is_sync
    }

    fn try_read_byte(&mut self) -> Option<u32> {
        let expected_track_id = self.track_handle.get_head_half_track();
        
        for _ in 0..8 {
            if self.pending_bit_count == 0 {
                let byte = self.track_handle.next_read_byte(expected_track_id, 0xFF, true);
                self.pending_bits = byte as u16;
                self.pending_bit_count = 8;
            }
            
            self.pending_bit_count -= 1;
            let bit = ((self.pending_bits >> self.pending_bit_count) & 1) as u8;
            
            if bit == 1 {
                self.sync_counter += 1;
            } else {
                self.sync_counter = 0;
            }
            
            if self.sync_counter >= 10 {
                let just_entered_sync = !self.is_sync;
                self.is_sync = true;
                self.bits_in_reg = 0;
                if just_entered_sync {
                    return Some(0xFFFFFFFF);
                }
            } else {
                if self.is_sync && bit == 0 {
                    self.is_sync = false;
                    self.bits_in_reg = 0;
                }
                
                if !self.is_sync {
                    self.bit_shift_reg = (self.bit_shift_reg << 1) | bit;
                    self.bits_in_reg += 1;
                }
            }
        }
        
        if self.bits_in_reg >= 8 {
            let output = self.bit_shift_reg;
            self.bits_in_reg -= 8;
            Some(output as u32)
        } else {
            None
        }
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
        // Misaligned SYNC test: 0x7F, 0xFF, 0xFE
        active_buf[0] = 0x7F; // 01111111 (7 ones)
        active_buf[1] = 0xFF; // 11111111 (8 ones) -> 15 ones total
        active_buf[2] = 0xFE; // 11111110 (7 ones, then a 0)
        active_buf[3] = 0x55; // 01010101
        active_buf[4] = 0x55;
        manager.commit_loaded_track(0, crate::track_buffer::TrackLayout::Accurate, 5);

        let mut surface = SoftwareMagneticSurface::new(emulator);
        surface.start_reading(0, 0);
        
        let out0 = surface.try_read_byte(); 
        assert_eq!(out0, Some(0x7F)); // The first byte is pushed normally

        let out1 = surface.try_read_byte();
        assert_eq!(out1, None); // We hit 10 ones, entered SYNC, no output

        assert_eq!(surface.is_sync_active(), true);

        let out2 = surface.try_read_byte(); // Reading 0xFE. At the end, hits 0, exits sync!
        assert_eq!(surface.is_sync_active(), false);
        assert_eq!(out2, None); // The 0 is shifted in, but we don't have 8 bits yet.
    }
}
