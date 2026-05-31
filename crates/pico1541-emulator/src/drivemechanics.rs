use crate::via6522::Via6522;
use pico1541_common::{ActiveTrack, formats::TrackLayout, interface::{DriveEvent, HostInterface}};

// 35 tracks + 7 tracks more that can be physically reached (MAX 42)
// NOTE! Min track is -2, since the physical head can move slightly beyond the 
//       outer most track of the disk before hitting the physical head-stop.
const MIN_HALF_TRACK: i8 = -2;
const MAX_HALF_TRACK: i8 = (35 + 7) * 2;
const INITIAL_HALF_TRACK: i8 = 34; // Start at track 18 (Half-track 34)
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
    pub halftrack_head_pos: i8,
    
    /// The active track we are reading and writing from
    track_byte_size: usize,
    track_byte_pos_ix: usize,
    
    byte_cycle_counter: u32,
    byte_cycle_limit: u32,

    last_state: u8,
    last_requested_sector: u8,
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
            last_requested_sector: 0xFF,
        }
    }
}

impl Default for DriveMechanics {
    fn default() -> Self {
        Self::new()
    }
}

impl DriveMechanics {
    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.track_byte_size = DEFAULT_BYTE_LENGTH;
        self.track_byte_pos_ix = 0;
        self.byte_cycle_counter = 0;
        self.byte_cycle_limit = CYCLES_ZONE_0;
        self.last_state = 0;
        self.last_requested_sector = 0xFF;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU, and the SO pin should be set
    #[link_section = ".data"]
    #[inline(always)]
    pub fn tick<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522, requested_sector: u8) -> bool {
        // :? Is the motor off? (PB2)
        if (self.last_state & MASK_MOTOR) == 0 {
            return false;
        }

        // :? Did the byte-cycle counter reach the limit?
        self.byte_cycle_counter += 1;
        if self.byte_cycle_counter < self.byte_cycle_limit {
            return false;
        }

        // E-> Yes, reset and process incoming byte
        self.byte_cycle_counter = 0;
        self.tick_slow(hardware, via2, requested_sector)
    }

    #[link_section = ".data"]
    #[inline(never)]
    fn tick_slow<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522, requested_sector: u8) -> bool {
        // Get the active track from hardware
        let track = hardware.get_active_track();
        track.current_byte_idx = self.track_byte_pos_ix as u16;

        // Advance the track position
        let gcr_pos = self.resolve_track_byte_position(track, requested_sector);

        // Determine read mode from CB2 (Manual High = Read)
        let is_read_mode = via2.get_cb2_out().unwrap_or(true);

        // :? Is Read mode enabled?
        if is_read_mode {
            // -> Yes, then get the byte
            let gcr_byte = gcr_pos
                    .map(|ix| track.gcr_data[ix])
                    .unwrap_or(BYTE_VOID);

            // SYNC signal is Active LOW (0 = SYNC found, 1 = No SYNC)
            if gcr_byte == BYTE_SYNC {
                via2.in_b &= !MASK_SYNC; // Assert SYNC (Pull PB7 Low)
            } else {
                via2.in_b |= MASK_SYNC;  // Release SYNC (Let PB7 float High)
            }                

            via2.in_a = gcr_byte;
        } 
        else if let Some(ix) = gcr_pos {
            // WRITE mode: take from ORA and put to disk
            track.gcr_data[ix] = via2.ora;
            track.is_dirty = true;
        }

        // Signal CA1 and SO ONLY if SYNC is NOT active.
        // On real 1541 hardware, the bit-shifter is held while SYNC is asserted,
        // so no Byte Ready pulses are generated during SYNC.
        let sync_active = (via2.in_b & MASK_SYNC) == 0;
        if !sync_active {
            // Signal CA1 on every byte ready
            via2.ca1_irq();

            // Trigger SO pulse (stay active for 1 cycle to ensure CPU sees it)
            // 1541-II hardware uses CA2=HIGH to ENABLE SO signals.
            let is_so_enabled = via2.get_ca2_out().unwrap_or(false);
            return is_so_enabled;
        }   

        false
    }

    /// Updates the drive mechanics state based on VIA2 Port B outputs.
    #[link_section = ".data"]
    pub fn update_state<H: HostInterface>(&mut self, orb: u8, ddrb: u8, hardware: &mut H) {
        let full_state = orb & ddrb & (MASK_HARDWARE_STATE | MASK_SPEED_ZONE);
        if full_state != self.last_state {
            self.update_state_slow(full_state, hardware);
        }
    }

    pub fn half_track(&self) -> u8 {
        // Do not expose the physical half-track outside the drive mechanics.
        self.halftrack_head_pos.clamp(0, MAX_HALF_TRACK) as u8
    }

    pub fn track_byte_pos_ix(&self) -> usize {
        self.track_byte_pos_ix
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

    /// Updates the track current position, and returns the position in the buffer
    #[link_section = ".data"]
    #[inline(always)]
    fn resolve_track_byte_position(&mut self, track: &ActiveTrack, requested_sector: u8) -> Option<usize> {
        let layout = if self.halftrack_head_pos == track.half_track as i8 {
            track.layout
        } else {
            TrackLayout::Void
        };

        let has_position = match layout {
            TrackLayout::Accurate(size) => {
                // :? Has the track size changed?    
                if size != self.track_byte_size && self.track_byte_size > 0 {
                    // -> Yes, then calculate the new track byte position
                    let new_track_position = (self.track_byte_pos_ix * size) 
                            / self.track_byte_size;

                    emu_debug!("[Accurate] Recalculated track [{}] position: {} / {} -> {} / {}", 
                            self.halftrack_head_pos, self.track_byte_pos_ix, self.track_byte_size, new_track_position, size);
                    
                    // Update the byte position and size
                    self.track_byte_pos_ix = new_track_position;
                    self.track_byte_size = size;
                }
                
                true
            },
            TrackLayout::Warp(size) => {
                self.track_byte_size = size;
                true
            },
            TrackLayout::StriddenWarp(size, stride) => {
                self.track_byte_size = size;
                if requested_sector != self.last_requested_sector {
                    emu_info!("[StriddenWarp] Warping from sector {} to {} (stride={}, pos={} -> {})", 
                        self.last_requested_sector, requested_sector, stride, self.track_byte_pos_ix, (requested_sector as usize) * stride);
                    self.track_byte_pos_ix = (requested_sector as usize) * stride;
                    self.last_requested_sector = requested_sector;
                }
                true
            },
            TrackLayout::Void => {
                self.last_requested_sector = 0xFF; // Reset sector tracking on void track
                false
            }
        };

        // :? Is track position past the end of the track?
        if self.track_byte_pos_ix >= self.track_byte_size {
            // -> Yes, then reset the track position
            self.track_byte_pos_ix = 0;
        }

        // :: Get the current track position, and advance by 1 byte
        let current_byte_pos = self.track_byte_pos_ix;
        self.track_byte_pos_ix += 1;

        // :? Is there a valid track position
        if has_position {
            // -> Yes, to return it
            Some(current_byte_pos)
        } else {
            // -> No, then return None
            None
        }
    }

    /// Updates the drive mechanics state, and issues events accordingly.
    #[link_section = ".data"]
    #[inline(never)]
    fn update_state_slow(&mut self, full_state: u8, hardware: &mut impl HostInterface) {
        let last_state = self.last_state;
        let changed = last_state ^ full_state;
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
            self.byte_cycle_counter = 0;
        }

        // Check head stepping
        let is_track_step = changed & MASK_STEP != 0;
        if is_track_step {
            let last_phase = last_state & MASK_STEP;
            let new_phase = full_state & MASK_STEP;
            let delta = (new_phase.wrapping_sub(last_phase) & MASK_STEP) as i8;
            
            let last_half_track_pos = self.halftrack_head_pos;
            let step = match delta {
                1 => 1,  // Step IN
                3 => -1, // Step OUT
                _ => 0,
            };

            self.halftrack_head_pos = (last_half_track_pos + step)
                    .clamp(MIN_HALF_TRACK, MAX_HALF_TRACK);

            if self.halftrack_head_pos >= 0 && self.halftrack_head_pos <= MAX_HALF_TRACK {
                hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.halftrack_head_pos as u8} );
            }
        }
    }
}
