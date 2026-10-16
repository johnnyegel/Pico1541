use crate::track_buffer::{Lfsr16, TrackBuffer, MAX_WEAK_RANGES, TRACK_SIZE};

pub trait MagneticSurface {
    /// Returns whether a SYNC mark is currently active.
    /// Default implementation returns `false` — PIO-based surfaces communicate
    /// sync state exclusively through try_read_byte() sentinel values.
    fn is_sync_active(&self) -> bool {
        false
    }
    fn try_read_byte(&mut self, delta_cycles: u32) -> Option<u8>;
    fn get_absolute_head_position(&self, master_clock: u32) -> u32;
    fn set_motor_running(&mut self, is_running: bool, master_clock: u32);
    fn set_speed_zone(&mut self, zone: u8);
    fn set_stream_source(&mut self, stream: &'static TrackBuffer, master_clock: u32);
    fn poll_revolution(&mut self) {}
}

pub struct SoftwareMagneticSurface {
    stream: Option<&'static TrackBuffer>,
    stream_idx: usize,
    is_sync: bool,
    bit_shift_reg: u8,
    bits_in_reg: u8,
    sync_counter: u8,
    pending_bits: u16,
    pending_bit_count: u8,
    cycle_counter: u32,
    cycle_limit: u32,
    lfsr: Lfsr16,
}

impl Default for SoftwareMagneticSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftwareMagneticSurface {
    pub fn new() -> Self {
        Self {
            stream: Some(TrackBuffer::noise()),
            stream_idx: 0,
            is_sync: false,
            bit_shift_reg: 0,
            bits_in_reg: 0,
            sync_counter: 0,
            pending_bits: 0,
            pending_bit_count: 0,
            cycle_counter: 0,
            cycle_limit: 28,
            lfsr: Lfsr16::new(),
        }
    }

    pub fn mutate_weak_ranges(&mut self) {
        if let Some(stream) = self.stream {
            if stream.data.is_null() {
                return;
            }
            let data = unsafe { &mut *stream.data };
            let count = (stream.weak_range_count as usize).min(MAX_WEAK_RANGES);
            if count > 0 {
                for range in stream.weak_ranges[..count].iter().flatten() {
                    let start = (range.offset as usize).min(stream.length).min(TRACK_SIZE);
                    let end = ((range.offset as usize) + (range.length as usize))
                        .min(stream.length)
                        .min(TRACK_SIZE);
                    for byte in &mut data[start..end] {
                        *byte = self.lfsr.next_weak_byte();
                    }
                }
            }
        }
    }
}

impl MagneticSurface for SoftwareMagneticSurface {
    fn poll_revolution(&mut self) {
        self.mutate_weak_ranges();
    }

    fn is_sync_active(&self) -> bool {
        self.is_sync
    }

    fn try_read_byte(&mut self, delta_cycles: u32) -> Option<u8> {
        self.cycle_counter += delta_cycles;
        if self.cycle_counter < self.cycle_limit {
            return None;
        }
        self.cycle_counter -= self.cycle_limit;

        let mut assembled_byte = None;

        for _ in 0..8 {
            if self.pending_bit_count == 0 {
                let byte = if let Some(stream) = self.stream {
                    let stream_len = stream.length.min(crate::track_buffer::TRACK_SIZE);
                    if !stream.data.is_null() && stream_len > 0 {
                        let b = unsafe { (*stream.data)[self.stream_idx] };
                        let old_idx = self.stream_idx;
                        self.stream_idx = (self.stream_idx + 1) % stream_len;
                        if self.stream_idx == 0 && old_idx > 0 {
                            self.mutate_weak_ranges();
                        }
                        b
                    } else {
                        0x55
                    }
                } else {
                    0x55
                };
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
                self.is_sync = true;
                self.bits_in_reg = 0;
            } else {
                if self.is_sync && bit == 0 {
                    self.is_sync = false;
                    self.bits_in_reg = 0;
                }

                if !self.is_sync {
                    self.bit_shift_reg = (self.bit_shift_reg << 1) | bit;
                    self.bits_in_reg += 1;

                    if self.bits_in_reg == 8 {
                        assembled_byte = Some(self.bit_shift_reg);
                        self.bits_in_reg = 0;
                    }
                }
            }
        }

        assembled_byte
    }

    fn get_absolute_head_position(&self, _master_clock: u32) -> u32 {
        self.stream_idx as u32
    }

    fn set_motor_running(&mut self, _is_running: bool, _master_clock: u32) {
        // Software surface is implicitly always running or stopped by the emulator logic
    }

    fn set_speed_zone(&mut self, expected_speed_zone: u8) {
        self.cycle_limit = match expected_speed_zone {
            3 => 26, // CYCLES_ZONE_3
            2 => 28, // CYCLES_ZONE_2
            1 => 30, // CYCLES_ZONE_1
            _ => 32, // CYCLES_ZONE_0
        };
    }

    fn set_stream_source(&mut self, stream: &'static TrackBuffer, _master_clock: u32) {
        self.stream = Some(stream);
        self.stream_idx = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track_buffer::DualCoreTrackBuffer;

    #[test]
    fn test_is_sync_logic() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        let active_buf = manager.start_track_modification();
        // Misaligned SYNC test: 0x7F, 0xFF, 0xFE
        active_buf[0] = 0x7F; // 01111111 (7 ones)
        active_buf[1] = 0xFF; // 11111111 (8 ones) -> 15 ones total
        active_buf[2] = 0xFE; // 11111110 (7 ones, then a 0)
        active_buf[3] = 0x55; // 01010101
        active_buf[4] = 0x55;
        manager.commit_loaded_track(
            crate::track_buffer::TrackIdentifier(0),
            crate::track_buffer::TrackLayout::Accurate,
            5,
        );

        let mut surface = SoftwareMagneticSurface::new();
        let stream = emulator.get_active_track_buffer();
        surface.set_stream_source(stream, 0);
        surface.set_motor_running(true, 0);

        let out0 = surface.try_read_byte(32);
        assert_eq!(out0, Some(0x7F)); // The first byte is pushed normally
        assert!(!surface.is_sync_active());

        let out1 = surface.try_read_byte(32);
        assert_eq!(out1, None); // Now just None, sync_active goes true immediately!
        assert!(surface.is_sync_active());

        // Next bytes should read out normal bytes
        let out2 = surface.try_read_byte(32); // Reading 0xFE. At the end, hits 0, exits sync!
        assert!(!surface.is_sync_active());
        assert_eq!(out2, None); // The 0 is shifted in, but we don't have 8 bits yet.
    }
}
