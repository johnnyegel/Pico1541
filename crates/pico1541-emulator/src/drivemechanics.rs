use crate::via6522::Via6522;
use pico1541_common::{magnetic_surface::MagneticSurface, interface::{DriveEvent, HostInterface}};

// The read head steps in half-track increments, so we represent the head position in half-tracks (0.5 track steps).
// We also keep a half-track position for the head, where Half-track 0 corresponds to Track 0, 
// Half-track -2 corresponds to Track 0, Half-track 0 corresponds to Track 1, and so on.
// Track 18 (the directory track) then corresponds to Half-track 36 (2 * 18).
// NOTE! Min track is -2, which represent a non-existing Track 0. This accomodates the fact that
//       the head can be stepped outwards beyond Track 1.
const MIN_HALF_TRACK: i8 = -2;
const MAX_HALF_TRACK: i8 = 85; // Half-track 85 corresponds to Track 42.5, which is the maximum physical reach of the head on a 1541 drive.
const INITIAL_HALF_TRACK: i8 = 34; // Start at track 18 (Half-track 34)

// Speed Zones (Cycles per byte @ 1MHz)
const CYCLES_ZONE_3: u32 = 26; // Tracks 1-17
const CYCLES_ZONE_2: u32 = 28; // Tracks 18-24
const CYCLES_ZONE_1: u32 = 30; // Tracks 25-30
const CYCLES_ZONE_0: u32 = 32; // Tracks 31-35

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
pub struct DriveMechanics<S: MagneticSurface> {
    /// The Track the head is positioned over (not necessarily the track being read)
    pub halftrack_head_pos: i8,
    
    pub surface: S,
    
    byte_cycle_counter: u32,
    byte_cycle_limit: u32,

    last_state: u8,
    last_gcr_byte: u8,
    last_requested_sector: u8,
    so_pulse_cycles: u8,
    is_sync: bool,
}

impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn new(surface: S) -> Self {
        Self {
            halftrack_head_pos: INITIAL_HALF_TRACK,
            surface,
            byte_cycle_counter: 0,
            byte_cycle_limit: CYCLES_ZONE_2, // Track 18 default
            last_state: 0,
            last_gcr_byte: 0,
            last_requested_sector: 0xFF,
            so_pulse_cycles: 0,
            is_sync: false,
        }
    }
}



impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.byte_cycle_counter = 0;
        self.byte_cycle_limit = CYCLES_ZONE_0; // Track 1 default
        self.last_state = 0;
        self.last_requested_sector = 0xFF;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU, and the SO pin should be set
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick_cycles<H: HostInterface>(&mut self, hardware: &mut H, _via1: &mut crate::via6522::Via6522, via2: &mut Via6522, requested_sector: u8, delta: u32) -> bool {
        if delta == 0 { return false; }
        // :? Is the motor off? (PB2)
        if (self.last_state & MASK_MOTOR) == 0 {
            return false;
        }

        let mut pulsed = false;
        if self.so_pulse_cycles > 0 {
            if delta >= self.so_pulse_cycles as u32 {
                self.so_pulse_cycles = 0;
                hardware.debug_pin_1(false);
            } else {
                self.so_pulse_cycles -= delta as u8;
            }
            pulsed = self.so_pulse_cycles > 0;
        }

        // :? Did the byte-cycle counter reach the limit?
        self.byte_cycle_counter += delta;
        if self.byte_cycle_counter < self.byte_cycle_limit {
            return pulsed;
        }

        // E-> Yes, reset and process incoming byte
        // Actually subtract the limit, in case delta jumped past it
        self.byte_cycle_counter -= self.byte_cycle_limit;
        
        // Remember if we were already pulsing to avoid double-triggering the CPU
        let was_pulsing = self.so_pulse_cycles > 0;
        
        self.tick_slow(hardware, via2, requested_sector);
        
        // Only return true (trigger CPU SO) if we just started a new pulse this tick
        !was_pulsing && self.so_pulse_cycles > 0
    }

    #[link_section = ".data.ram_func"]
    fn tick_slow<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522, _requested_sector: u8) {
        let is_read_mode = via2.get_cb2_out().unwrap_or(true);
        let is_so_enabled = via2.get_ca2_out().unwrap_or(false);
        let is_reading = is_read_mode && is_so_enabled;
        let mut suppress_ca1 = false;

        if is_read_mode {
            if is_reading {
                if let Some(word) = self.surface.try_read_byte() {
                    if word == 0xFFFFFFFF {
                        self.is_sync = true;
                    } else {
                        self.is_sync = false;
                        let gcr_byte = word as u8;
                        via2.in_a = gcr_byte;
                        self.last_gcr_byte = gcr_byte;
                    }
                }
            }
            
            if self.is_sync {
                via2.in_b &= !MASK_SYNC; // Assert SYNC (Pull PB7 Low)
                hardware.debug_pin_0(true);
                suppress_ca1 = true;
            } else {
                via2.in_b |= MASK_SYNC;  // Release SYNC
                hardware.debug_pin_0(false);
            }
        } else {
            // WRITE mode
            self.surface.stop_reading();
            // We use 0 as master clock for now, as host tests don't have a reliable master clock
            let index = self.surface.get_absolute_head_position(0);
            self.surface.write_byte_at(index, via2.ora);
        }

        let sync_active = (via2.in_b & MASK_SYNC) == 0;
        if !sync_active && !suppress_ca1 {
            via2.ca1_irq();
            self.so_pulse_cycles = 3; 
            hardware.debug_pin_1(true);
        }   
    }

    /// Updates the drive mechanics state based on VIA2 Port B outputs.
    #[link_section = ".data.ram_func"]
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
        self.surface.get_absolute_head_position(0) as usize
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

    /// Updates the drive mechanics state, and issues events accordingly.
    #[link_section = ".data.ram_func"]
    #[inline(never)]
    fn update_state_slow(&mut self, full_state: u8, hardware: &mut impl HostInterface) {
        let last_state = self.last_state;
        let state_changes = last_state ^ full_state;
        self.last_state = full_state;

        // Check motor state change
        if state_changes & MASK_MOTOR != 0 {
            hardware.send_event(DriveEvent::MotorChanged(full_state & MASK_MOTOR != 0));
        }

        // Check LED state change
        if state_changes & MASK_LED != 0 {
            hardware.send_event(DriveEvent::LedChanged(full_state & MASK_LED != 0));
        }

        // 4. Update stepper motor and tracking
        // Did speed-zone change? (PB5, PB6)
        if (state_changes & MASK_SPEED_ZONE) != 0 {
            let speed_zone = (full_state & MASK_SPEED_ZONE) >> SHIFT_SPEED_ZONE;
            
            self.byte_cycle_limit = match speed_zone {
                3 => CYCLES_ZONE_3,
                2 => CYCLES_ZONE_2,
                1 => CYCLES_ZONE_1,
                _ => CYCLES_ZONE_0,
            };
        }
        
        let is_track_step = state_changes & MASK_STEP != 0;
        if is_track_step {
            let last_phase = last_state & MASK_STEP;
            let new_phase = full_state & MASK_STEP;
            let delta = (new_phase.wrapping_sub(last_phase) & MASK_STEP) as i8;
            
            let last_half_track_pos = self.halftrack_head_pos;
            let step: i8 = match delta {
                1 => 1,  // Step IN
                3 => -1, // Step OUT
                _ => 0,
            };

            self.halftrack_head_pos = (last_half_track_pos + step)
                    .clamp(MIN_HALF_TRACK, MAX_HALF_TRACK);

            if self.halftrack_head_pos >= 0 && self.halftrack_head_pos <= MAX_HALF_TRACK {
                self.surface.start_reading(self.halftrack_head_pos as u8, 0);
                hardware.track_changed(self.halftrack_head_pos as u8);
            }
        }
    }
}
