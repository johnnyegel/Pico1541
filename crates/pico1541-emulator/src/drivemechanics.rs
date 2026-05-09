use crate::via6522::Via6522;
use pico1541_common::{ActiveTrack, formats::TrackLayout, interface::{DriveEvent, HostInterface}};

// 35 tracks + 7 tracks more that can be physically reached (MAX 42)
const MAX_HALF_TRACK: u8 = (35 + 7) * 2;
const INITIAL_HALF_TRACK: u8 = 0; // Start at track 1 (half-track 0)
const DEFAULT_BYTE_LENGTH: usize = 7692;

const BYTE_SYNC: u8 = 0xFF;
const BYTE_VOID: u8 = 0x55;

// Speed Zones (Cycles per byte @ 1MHz)
const CYCLES_ZONE_0: u32 = 32; // Tracks 1-17
const CYCLES_ZONE_1: u32 = 30; // Tracks 18-24
const CYCLES_ZONE_2: u32 = 28; // Tracks 25-30
const CYCLES_ZONE_3: u32 = 26; // Tracks 31-35

// Masks for individual hardware state bits on VIA2 Port B
const MASK_MOTOR: u8 = 0x04;
const MASK_LED: u8 = 0x08;
const MASK_STEP: u8 = 0x03;

// Mask used to determine the last state of stepper motor phases (PB0, PB1), MOTOR bit and ACT LED and (PB2, PB3)
const MASK_HARDWARE_STATE: u8 = 0x0F;
const MASK_SPEED_ZONE: u8 = 0x60;
const MASK_SYNC: u8 = 0x80;
const SHIFT_SPEED_ZONE: u8 = 5;

/// DriveMechanics struct to represent the state of the drive's mechanical components, such as the head position and motor state.
pub struct DriveMechanics {
    /// The Track the head is positioned over (not necessarily the track being read)
    pub halftrack_head_pos: u8,
    
    /// The active track we are reading and writing from
    track_byte_size: usize,
    track_byte_pos_ix: usize,
    
    byte_cycle_counter: u32,
    byte_cycle_limit: u32,

    last_state: u8,
    so_pulse_remaining: u32,
}

impl DriveMechanics {
    pub fn new() -> Self {
        Self {
            halftrack_head_pos: INITIAL_HALF_TRACK,
            track_byte_size: DEFAULT_BYTE_LENGTH,
            track_byte_pos_ix: 0,
            byte_cycle_counter: 0,
            byte_cycle_limit: CYCLES_ZONE_0, // Track 1 default
            last_state: 0,
            so_pulse_remaining: 0,
        }
    }

    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.track_byte_size = DEFAULT_BYTE_LENGTH;
        self.track_byte_pos_ix = 0;
        self.byte_cycle_counter = 0;
        self.byte_cycle_limit = CYCLES_ZONE_0;
        self.last_state = 0;
        self.so_pulse_remaining = 0;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU, and the SO pin should be set
    pub fn tick<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522) -> bool {
        // :: Update the drive state and issue events if there are changes
        let is_motor_on = self.update_state(via2, hardware);

        // Get the active track from hardware
        let track = hardware.get_active_track();

        // :: Handle the start of a new byte
        if self.byte_cycle_counter == 0 {
            // :: Peak at the next byte to update SYNC state early
            let next_gcr_byte = self.resolve_track_byte_position_peek(&track)
                    .map(|ix| track.gcr_data[ix])
                    .unwrap_or(BYTE_VOID);

            // SYNC signal is Active LOW (0 = SYNC found, 1 = No SYNC)
            if is_motor_on && next_gcr_byte == BYTE_SYNC {
                via2.in_b &= !MASK_SYNC; // Assert SYNC (Pull PB7 Low)
            } else {
                via2.in_b |= MASK_SYNC;  // Release SYNC (Let PB7 float High)
            }
        }

        // :? Is the motor off?
        if !is_motor_on {
            return false;
        }

        // :: Update the sub-cycle counter
        self.byte_cycle_counter += 1;

        // :? Did the byte-cycle counter reach the limit?
        if self.byte_cycle_counter >= self.byte_cycle_limit {
            // E-> Yes, reset and signal byte ready
            self.byte_cycle_counter = 0;

            // Advance the track position
            let gcr_byte = self.resolve_track_byte_position(&track)
                    .map(|ix| track.gcr_data[ix])
                    .unwrap_or(BYTE_VOID);

            // Determine read mode from CB2 (Manual High = Read)
            let cb2_output = via2.get_cb2_out();
            let is_read_mode = cb2_output == Some(true);

            // If in read mode, put the byte in VIA2 Port A
            if is_read_mode {
                via2.in_a = gcr_byte;
            }

            // Signal CA1 and SO ONLY if SYNC is NOT active.
            // On real 1541 hardware, the bit-shifter is held while SYNC is asserted,
            // so no Byte Ready pulses are generated during SYNC.
            let sync_active = (via2.in_b & MASK_SYNC) == 0;
            if !sync_active {
                // Signal CA1 on every byte ready
                via2.ca1_irq();

                // Trigger SO pulse (stay active for 2 cycles to ensure CPU sees it)
                self.so_pulse_remaining = 2;
            }
        }

        // :: Handle SO pulse signaling
        if self.so_pulse_remaining > 0 {
            self.so_pulse_remaining -= 1;
            
            // 1541-II hardware uses CA2=HIGH to ENABLE SO signals.
            let ca2_output = via2.get_ca2_out().unwrap_or(false);
            let is_so_enabled = ca2_output;
            
            if is_so_enabled {
                return true;    // Signal SO to the CPU
            }
        }

        return false;
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self, via2: &Via6522) -> bool {
        (via2.orb & MASK_LED) != 0
    }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self, via2: &Via6522) -> bool {
        (via2.orb & MASK_MOTOR) != 0
    }

    // ========== Internal helper functions ==================================================

    /// Peak at the next track position without advancing
    #[inline(always)]
    fn resolve_track_byte_position_peek(&self, track: &ActiveTrack) -> Option<usize> {
        // :? Does the track match the head position?
        if self.halftrack_head_pos == track.half_track {
            if let TrackLayout::Accurate(_) = track.layout {
                if self.track_byte_pos_ix < 10000 {
                    return Some(self.track_byte_pos_ix);
                }
            }
            if let TrackLayout::Warp(_) = track.layout {
                if self.track_byte_pos_ix < 10000 {
                    return Some(self.track_byte_pos_ix);
                }
            }
        }
        None
    }

    /// Advance the track position without reading
    #[inline(always)]
    fn resolve_track_byte_position_advance(&mut self) {
        self.track_byte_pos_ix += 1;
        
        // Hard clamp
        if self.track_byte_pos_ix >= 10000 {
            self.track_byte_pos_ix = 0;
        }

        if self.track_byte_size > 0 && self.track_byte_pos_ix >= self.track_byte_size {
            self.track_byte_pos_ix = 0;
        }
    }

    /// Updates the track current position, and returns the position in the buffer
    #[inline(always)]
    fn resolve_track_byte_position(&mut self, track: &ActiveTrack) -> Option<usize> {
        let layout = if self.halftrack_head_pos == track.half_track {
            track.layout
        } else {
            TrackLayout::Void
        };

        let has_position = match layout {
            TrackLayout::Accurate(size) => {
                if size != self.track_byte_size && self.track_byte_size > 0 {
                    self.track_byte_pos_ix = (self.track_byte_pos_ix * size) 
                            / self.track_byte_size;

                }
                self.track_byte_size = size;
                if self.track_byte_pos_ix >= self.track_byte_size || self.track_byte_pos_ix >= 10000 {
                    self.track_byte_pos_ix = 0;
                }
                true
            },
            TrackLayout::Warp(size) => {
                self.track_byte_size = size;
                true
            },
            TrackLayout::Void => false
        };

        let current_byte_pos = self.track_byte_pos_ix;
        self.resolve_track_byte_position_advance();

        if has_position {
            Some(current_byte_pos)
        } else {
            None
        }
    }

    /// Updates the drive mechanics state, and issues events accordingly.
    /// Returns the current motor state
    #[inline(always)]
    fn update_state(&mut self, via2: &mut Via6522, hardware: &mut impl HostInterface) -> bool {
        // Catch state changes on the relevant bits of VIA2 Port B
        let full_state = via2.orb & (MASK_HARDWARE_STATE | MASK_SPEED_ZONE);

        let last_state = self.last_state;
        let changed = self.last_state ^ full_state;
        if changed == 0 {
            return full_state & MASK_MOTOR != 0;
        }

        self.last_state = full_state;

        // Check motor state change
        if changed & MASK_MOTOR != 0 {
            hardware.send_event(DriveEvent::MotorChanged(full_state & MASK_MOTOR != 0));
        }

        // Check LED state change
        if changed & MASK_LED != 0 {
            hardware.send_event(DriveEvent::LedChanged(full_state & MASK_LED != 0));
        }

        // Check speed zone changes (VIA2 Port B bits 6-5)
        // Inverted mapping confirmed by 1541-II ROM traces:
        // %11 (3) = 32 cycles (Zone 0)
        // %10 (2) = 30 cycles (Zone 1)
        // %01 (1) = 28 cycles (Zone 2)
        // %00 (0) = 26 cycles (Zone 3)
        let is_speed_zone_changed = changed & MASK_SPEED_ZONE != 0;
        if is_speed_zone_changed {
            let new_speed_zone = (full_state & MASK_SPEED_ZONE) >> SHIFT_SPEED_ZONE;
            self.byte_cycle_limit = match new_speed_zone {
                3 => CYCLES_ZONE_0, 
                2 => CYCLES_ZONE_1, 
                1 => CYCLES_ZONE_2, 
                0 => CYCLES_ZONE_3, 
                _ => CYCLES_ZONE_0,
            };
        }

        // Check head stepping
        let is_track_step = changed & MASK_STEP != 0;
        if is_track_step {
            let last_phase = last_state & MASK_STEP;
            let new_phase = full_state & MASK_STEP;

            let delta = (new_phase.wrapping_sub(last_phase) & MASK_STEP) as i8;
            
            // Stepper Direction Inverted based on 1541-II ROM traces:
            // Delta 1 (00->01) is OUT (towards Track 1)
            // Delta 3 (01->00) is IN (towards Track 35)
            self.halftrack_head_pos = match delta {
                3 => self.halftrack_head_pos.saturating_add(1).clamp(0, MAX_HALF_TRACK),  // Step IN
                1 => self.halftrack_head_pos.saturating_sub(1).clamp(0, MAX_HALF_TRACK),  // Step OUT
                _ => self.halftrack_head_pos,
            };

            hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.halftrack_head_pos} );
        }

        full_state & MASK_MOTOR != 0
    }
}
