use crate::via6522::Via6522;
use pico1541_common::{ActiveTrack, formats::TrackLayout, interface::{DriveEvent, HostInterface}};

// 35 tracks + 7 tracks more that can be physically reached (MAX 42)
const MAX_HALF_TRACK: u8 = (35 + 7) * 2;
const INITIAL_HALF_TRACK: u8 = 34; // Start at track 18 (half-track 34)
const DEFAULT_BYTE_LENGTH: usize = 7142;

const BYTE_SYNC: u8 = 0xFF;
const BYTE_VOID: u8 = 0x55;

// Speed Zones based on the VIA2 outputs
const BYTE_SPEED_ZONE_0: u32 = 32;
const BYTE_SPEED_ZONE_1: u32 = 30;
const BYTE_SPEED_ZONE_2: u32 = 28;
const BYTE_SPEED_ZONE_3: u32 = 26;

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
}

impl DriveMechanics {
    pub fn new() -> Self {
        Self {
            halftrack_head_pos: INITIAL_HALF_TRACK,
            track_byte_size: DEFAULT_BYTE_LENGTH,
            track_byte_pos_ix: 0,
            byte_cycle_counter: 0,
            byte_cycle_limit: BYTE_SPEED_ZONE_0,
            last_state: 0,
        }
    }

    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.track_byte_size = DEFAULT_BYTE_LENGTH;
        self.track_byte_pos_ix = 0;
        self.byte_cycle_counter = 0;
        self.byte_cycle_limit = BYTE_SPEED_ZONE_0;
        self.last_state = 0;
        
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU, and the SO pin should be set
    pub fn tick<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522) -> bool {
        // :: Update the drive state and issue events if there are changes
        let is_motor_on = self.update_state(via2, hardware);

        // :? Is the motor not yet spun up?
        if !is_motor_on {
            // -> No, then reset the sub-cycle counter so that we start counting fresh towards the next spin-up.
            via2.in_b &= !MASK_SYNC; // Clear sync pulse
            return false;
        }

        // :: Update the sub-cycle counter
        self.byte_cycle_counter += 1;
        // :? Did the byte-cycle counter wrap?
        if self.byte_cycle_counter < self.byte_cycle_limit {
            // -> No, then simply return and wait for the next tick
            return false;
        }
        
        // E-> Yes, then reset the sub-cycle counter and proceed to deliver the next byte
        self.byte_cycle_counter = 0;

        // :: Get the active track from hardware, and determine the layout based on the current half-track position.
        let track = hardware.get_active_track();
        
        // :: Determine the current operation mode
        // Determine read/from cb2 output:
        let cb2_output = via2.get_cb2_out();

        // Read mode: Some(true), Write mode: Some(false), None: bith false
        let is_read_mode = cb2_output == Some(true);
        let is_write_mode = cb2_output == Some(false);
        
        // Determine SO enabled mode
        let is_so_enabled = via2.get_ca2_out().unwrap_or(false);

        // :? Is Read mode enabled?
        if is_read_mode {
            // :: Update the track byte position and read the gcr byte
            let read_gcr_byte = self.resolve_track_byte_position(&track)
                    .map(|ix| track.gcr_data[ix])
                    .unwrap_or(BYTE_VOID);
            
            // Set the byte to VIA2 PA
            via2.in_a = read_gcr_byte;

            // :? Is this a sync byte?
            if read_gcr_byte == BYTE_SYNC {
                // -> Yes, then flag sync to via2
                via2.in_b |= MASK_SYNC;

                // We can return as there should be no byte ready pulse
                return false;
            }
            else {
                via2.in_b &= !MASK_SYNC;
            }


        }
        // :? Is write mode enabled?
        else if is_write_mode {
            // TODO! We currently do not do write!

        }

        // Finally handle byte ready signalling and return the SO state
        via2.ca1_irq();

        // The SO signals if the CPU SO pin should be triggered
        return is_so_enabled;
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

    /// Updates the track current position, and returns the position in the buffer to read from or write to
    #[inline(always)]
    fn resolve_track_byte_position(&mut self, track: &ActiveTrack) -> Option<usize> {
        // :? Does the track match the head position?
        let layout = if self.halftrack_head_pos == track.half_track {
            // -> Yes, get track layout from track
            track.layout
        } else {
            // -> No, then we use void as the layout
            TrackLayout::Void
        };

        let has_position = match layout {
            TrackLayout::Accurate(size) => {
                if size != self.track_byte_size {
                    // Scale the byte position to the new track size
                    self.track_byte_pos_ix = (self.track_byte_pos_ix * size) 
                            / self.track_byte_size;

                }
                self.track_byte_size = size;
                true
            },
            TrackLayout::Warp(size) => {
                self.track_byte_size = size;
                true
            },
            TrackLayout::Void => false
        };

        // :: Capture the current byte position
        let current_byte_pos = self.track_byte_pos_ix;

        self.track_byte_pos_ix += 1;
        if self.track_byte_pos_ix > self.track_byte_size {
            self.track_byte_pos_ix -= self.track_byte_size;
        }

        // Return the position, if the track has one
        if has_position {
            Some(current_byte_pos)
        }
        else {
            None
        }
    }

    /// Updates the drive mechanics state, and issues events accordingly.
    /// Returns the current motor state
    #[inline(always)]
    fn update_state(&mut self, via2: &mut Via6522, hardware: &mut impl HostInterface) -> bool {
        // Catch state changes on the relevant bits of VIA2 Port B (including bitrate)
        let full_state = via2.orb & (MASK_HARDWARE_STATE | MASK_SPEED_ZONE);

        // Has anything changed?
        let last_state = self.last_state;
        let changed = self.last_state ^ full_state;
        if changed == 0 {
            // No changes to handle, just return the motor state
            return full_state & MASK_MOTOR != 0;
        }

        self.last_state = full_state;

        // Check speed zone changes first
        let is_speed_zone_changed = changed & MASK_SPEED_ZONE != 0;
        if is_speed_zone_changed {
            // Update the speed zone
            let new_speed_zone = (full_state & MASK_SPEED_ZONE) >> SHIFT_SPEED_ZONE;
            self.byte_cycle_limit = match new_speed_zone {
                0 => BYTE_SPEED_ZONE_0,
                1 => BYTE_SPEED_ZONE_1,
                2 => BYTE_SPEED_ZONE_2,
                3 => BYTE_SPEED_ZONE_3,
                _ => BYTE_SPEED_ZONE_0,
            };
            emu_debug!("Speed zone changed: {}, cycles/byte: {}", new_speed_zone, self.byte_cycle_limit);
        }

        // Check head stepping
        let is_track_step = changed & MASK_STEP != 0;
        if is_track_step {
            // Get the previous and current phase
            let last_phase = last_state & MASK_STEP;
            let new_phase = full_state & MASK_STEP;

            // Determine the step direction based on the phase change
            // Anta at 'old_bits' er forrige verdi (0-3) og 'new_bits' er nåværende (0-3)
            let delta = (new_phase.wrapping_sub(last_phase) & MASK_STEP) as i8;
            self.halftrack_head_pos = match delta {
                1 => self.halftrack_head_pos.saturating_add(1).clamp(1, MAX_HALF_TRACK),  // Step in
                3 => self.halftrack_head_pos.saturating_sub(1).clamp(1, MAX_HALF_TRACK),  // Step out
                _ => self.halftrack_head_pos,  // No change or invalid transition (e.g., 0->2 or 1->3)
            };

            emu_debug!(
                    "HEAD Stepped: Phase {} -> {}, Half-Track: {}",
                    last_phase,
                    new_phase,
                    self.halftrack_head_pos);

            // Send step event
            hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.halftrack_head_pos} );
        }

        // :: Handle LED events
        if changed & MASK_LED != 0 {
            let led_state = full_state & MASK_LED != 0;
            hardware.send_event(DriveEvent::LedChanged(led_state));

            emu_debug!("LED changed state: {}", led_state);
        }

        // :: Handle Motor events
        if changed & MASK_MOTOR != 0 {
            let motor_state = full_state & MASK_MOTOR != 0;
            hardware.send_event(DriveEvent::MotorChanged(motor_state));

            emu_debug!("MOTOR changed state: {}", motor_state);
        }

        // Return the motor state
        return full_state & MASK_MOTOR != 0;
    }
}
