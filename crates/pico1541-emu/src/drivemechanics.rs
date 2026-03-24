use crate::via6522::Via6522;
use crate::DriveHardware;
use core::sync::atomic::Ordering;

/// Simulates the analog read/write head and motor logic.
pub struct DriveMechanics {
    /// Index of the current GCR byte being read/written under the head.
    pub current_byte_idx: usize,
    /// Internal counter to track CPU cycles between byte operations.
    pub sub_cycle_counter: u32,
    /// Last seen stepper phase (from VIA2 Port B bits 0-1) to detect steps.
    pub last_step_phase: u8,
    /// Current physical half-track position (0 to 84).
    pub current_half_track: u8,
}

impl DriveMechanics {
    pub fn new() -> Self {
        Self {
            current_byte_idx: 0,
            sub_cycle_counter: 0,
            last_step_phase: 0,
            current_half_track: 36, // Start at Track 18.0
        }
    }

    /// Returns true if a new byte was just processed by the disk head.
    pub fn tick<H: DriveHardware>(&mut self, hardware: &mut H, via2: &mut Via6522) -> bool {
        // 1. Handle Stepping (VIA2 Port B bits 0-1)
        let current_phase = via2.orb & 0x03;
        if current_phase != self.last_step_phase {
            let mut stepped = false;
            match (self.last_step_phase, current_phase) {
                (0, 1) | (1, 2) | (2, 3) | (3, 0) => {
                    if self.current_half_track < 84 {
                        self.current_half_track += 1;
                        stepped = true;
                    }
                }
                (0, 3) | (3, 2) | (2, 1) | (1, 0) => {
                    if self.current_half_track > 0 {
                        self.current_half_track -= 1;
                        stepped = true;
                    }
                }
                _ => {}
            }
            self.last_step_phase = current_phase;
            
            if stepped {
                hardware.notify_head_stepped(self.current_half_track);
            }
        }

        // 2. Handle Motor and Rotation
        // Motor state is managed by SystemBus notifications, but DriveMechanics 
        // needs it to know whether to rotate.
        let motor_on = (via2.orb & 0x04) != 0;

        // Fetch shared metadata
        let active_track = hardware.get_active_track();
        let disk_inserted = active_track.disk_inserted.load(Ordering::Relaxed);
        let bit_length = active_track.bit_length;

        if !motor_on || !disk_inserted {
            return false;
        }

        // 3. Bitrate and Disk Rotation
        let bitrate_bits = (via2.orb >> 5) & 0x03;
        let cycles_per_byte = match bitrate_bits {
            0 => 32,
            1 => 35,
            2 => 37,
            3 => 40,
            _ => 32,
        };

        self.sub_cycle_counter += 1;
        if self.sub_cycle_counter >= cycles_per_byte {
            self.sub_cycle_counter = 0;

            let track_bytes = (bit_length / 8) as usize;
            if track_bytes > 0 {
                self.current_byte_idx = (self.current_byte_idx + 1) % track_bytes;

                // VIA2 Port B bit 4: Read/Write mode
                let write_mode = (via2.orb & 0x10) != 0;

                if write_mode {
                    let gcr_data_ptr = active_track.gcr_data.as_ptr() as *mut u8;
                    unsafe {
                        *gcr_data_ptr.add(self.current_byte_idx) = via2.ora;
                    }
                    if !active_track.is_dirty.swap(true, Ordering::Relaxed) {
                        hardware.notify_track_dirty();
                    }
                    return true;
                } else {
                    let gcr_byte = active_track.gcr_data[self.current_byte_idx];
                    via2.in_a = gcr_byte;

                    if gcr_byte == 0xFF {
                        via2.in_b &= !0x80; // SYNC Pull LOW
                    } else {
                        via2.in_b |= 0x80;  // SYNC Release HIGH
                    }
                    return true;
                }
            }
        }

        false
    }
}
