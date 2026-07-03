use crate::track_buffer::EmulatorHandle;

pub trait MagneticSurface {
    /// Returns whether a SYNC mark is currently active.
    /// Default implementation returns `false` — PIO-based surfaces communicate
    /// sync state exclusively through try_read_byte() sentinel values.
    fn is_sync_active(&self) -> bool { false }
    fn try_read_byte(&mut self, delta_cycles: u32) -> Option<u32>;
    fn get_absolute_head_position(&self, master_clock: u32) -> u32;
    fn write_byte_at(&mut self, index: u32, data: u8);
    fn start_reading(&mut self, track_id: u8, rotation_fixed: u32);
    fn stop_reading(&mut self);
    fn is_write_protected(&self) -> bool;
    fn pop_force_disk_change(&mut self) -> bool;
    fn set_speed_zone(&mut self, zone: u8);
    fn inject_analog_noise(&mut self);
    fn get_active_speed_zone(&self) -> Option<u8>;
    fn get_active_track_length(&self) -> usize;
}

pub struct SoftwareMagneticSurface {
    pub track_handle: EmulatorHandle,
    is_sync: bool,
    bit_shift_reg: u8,
    bits_in_reg: u8,
    sync_counter: u8,
    pending_bits: u16,
    pending_bit_count: u8,
    cycle_counter: u32,
    cycle_limit: u32,
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
            cycle_counter: 0,
            cycle_limit: 28,
        }
    }
}

impl MagneticSurface for SoftwareMagneticSurface {
    fn is_sync_active(&self) -> bool {
        self.is_sync
    }

    fn try_read_byte(&mut self, delta_cycles: u32) -> Option<u32> {
        self.cycle_counter += delta_cycles;
        if self.cycle_counter < self.cycle_limit {
            return None;
        }
        self.cycle_counter -= self.cycle_limit;

        let expected_track_id = self.track_handle.get_head_half_track();
        let mut assembled_byte = None;
        let mut sync_trigger = false;
        
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
                    sync_trigger = true;
                }
            } else {
                if self.is_sync && bit == 0 {
                    self.is_sync = false;
                    self.bits_in_reg = 0;
                }
                
                if !self.is_sync {
                    self.bit_shift_reg = (self.bit_shift_reg << 1) | bit;
                    self.bits_in_reg += 1;
                    
                    if self.bits_in_reg == 8 {
                        assembled_byte = Some(self.bit_shift_reg as u32);
                        self.bits_in_reg = 0;
                    }
                }
            }
        }
        
        if sync_trigger {
            Some(0xFFFFFFFF)
        } else {
            assembled_byte
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

    fn set_speed_zone(&mut self, zone: u8) {
        self.cycle_limit = match zone {
            3 => 26, // CYCLES_ZONE_3
            2 => 28, // CYCLES_ZONE_2
            1 => 30, // CYCLES_ZONE_1
            _ => 32, // CYCLES_ZONE_0
        };
    }

    fn inject_analog_noise(&mut self) {
        // No-op for software surface. Test harnesses could use this to verify behavior.
    }

    fn get_active_speed_zone(&self) -> Option<u8> {
        self.track_handle.get_active_speed_zone()
    }

    fn get_active_track_length(&self) -> usize {
        self.track_handle.get_active_track_length()
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
        manager.commit_loaded_track(0, crate::track_buffer::TrackLayout::Accurate, 5, None);

        let mut surface = SoftwareMagneticSurface::new(emulator);
        surface.start_reading(0, 0);
        
        let out0 = surface.try_read_byte(32); 
        assert_eq!(out0, Some(0x7F)); // The first byte is pushed normally
        assert!(!surface.is_sync_active());

        let out1 = surface.try_read_byte(32);
        assert_eq!(out1, Some(0xFFFFFFFF));
        assert!(surface.is_sync_active());
        
        // Next bytes should read out normal bytes
        let out2 = surface.try_read_byte(32); // Reading 0xFE. At the end, hits 0, exits sync!
        assert!(!surface.is_sync_active());
        assert_eq!(out2, None); // The 0 is shifted in, but we don't have 8 bits yet.
    }
}
