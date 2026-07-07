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
    
    last_state: u8,
    last_gcr_byte: u8,
    last_requested_sector: u8,
    so_pulse_cycles: u32,
}

impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn new(surface: S) -> Self {
        Self {
            halftrack_head_pos: INITIAL_HALF_TRACK,
            surface,
            last_state: 0,
            last_gcr_byte: 0,
            last_requested_sector: 255,
            so_pulse_cycles: 0,
        }
    }
}



impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.last_state = 0;
        self.last_requested_sector = 0xFF;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick_cycles<H: HostInterface>(&mut self, hardware: &mut H, _via1: &mut crate::via6522::Via6522, via2: &mut Via6522, _requested_sector: u8, delta: u32) -> bool {
        if delta == 0 { return false; }
        // :? Is the motor off? (PB2)
        if (self.last_state & MASK_MOTOR) == 0 {
            return false;
        }

        let is_read_mode = via2.get_cb2_out().unwrap_or(true);
        let is_so_enabled = via2.get_ca2_out().unwrap_or(false);
        let mut suppress_ca1 = false;
        let mut so_pulsed = false;

        if is_read_mode {
            let mut new_byte_arrived = false;
            
            if let Some(byte) = self.surface.try_read_byte(delta) {
                via2.in_a = byte;
                self.last_gcr_byte = byte;
                new_byte_arrived = true;
            }
            
            let is_sync = self.surface.is_sync_active();
            
            if is_sync {
                via2.in_b &= !MASK_SYNC; // Assert SYNC (Pull PB7 Low)
                hardware.debug_pin_1(true); // Assert SYNC on debug pin
                suppress_ca1 = true;
            } else {
                via2.in_b |= MASK_SYNC;  // Release SYNC
                hardware.debug_pin_1(false); // Release SYNC on debug pin
            }
            
            let sync_active = (via2.in_b & MASK_SYNC) == 0;
            if !sync_active && !suppress_ca1 && new_byte_arrived && is_so_enabled {
                via2.ca1_irq();
                self.so_pulse_cycles = 3;
                hardware.debug_pin_0(true); // Assert SO pulse on debug pin
                so_pulsed = true;
            }

            if self.so_pulse_cycles > 0 {
                self.so_pulse_cycles = self.so_pulse_cycles.saturating_sub(delta);
                if self.so_pulse_cycles == 0 {
                    hardware.debug_pin_0(false); // Release SO pulse on debug pin
                }
            }
        } else {
            // WRITE mode
            self.surface.stop_reading();
            // We use 0 as master clock for now, as host tests don't have a reliable master clock
            let index = self.surface.get_absolute_head_position(0);
            self.surface.write_byte_at(index, via2.ora);
        }
        
        so_pulsed
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
            self.surface.set_speed_zone(speed_zone);
            
            if speed_zone != self.surface.get_active_speed_zone().unwrap_or(speed_zone) {
                self.surface.inject_analog_noise();
            }
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

            if step == -1 && last_half_track_pos <= MIN_HALF_TRACK {
                log::error!("HEAD BUMP DETECTED! last_half_track_pos={}, delta={}, full_state={:02X}", last_half_track_pos, delta, full_state);
                #[cfg(feature = "flight-recorder")]
                unsafe {
                    let ptr = core::ptr::addr_of_mut!(crate::flight_recorder::FLIGHT_RECORDER) as *mut crate::flight_recorder::FlightRecorder;
                    log::error!("--- FLIGHT RECORDER DUMP ---");
                    let start_idx = (*ptr).index;
                    for i in 0..512 {
                        let idx = (start_idx + i) % 512;
                        let rec = &(*ptr).buffer[idx];
                        if rec.pc != 0 {
                            log::error!("FR[{}]: Cycle={}, PC={:04X}, VIA1={:02X}, VIA2={:02X}, PCR1={:02X}, PCR2={:02X}", 
                                i, rec.cycle, rec.pc, rec.via1_pb, rec.via2_pb, rec.pcr1, rec.pcr2);
                        }
                    }
                    log::error!("--- END FLIGHT RECORDER DUMP ---");
                }
            }

            self.halftrack_head_pos = (last_half_track_pos + step)
                    .clamp(MIN_HALF_TRACK, MAX_HALF_TRACK);

            if self.halftrack_head_pos >= 0 && self.halftrack_head_pos <= MAX_HALF_TRACK {
                self.surface.start_reading(self.halftrack_head_pos as u8, 0);
                
                if self.surface.get_active_track_length() == 0 {
                    self.surface.inject_analog_noise();
                }
                
                hardware.track_changed(self.halftrack_head_pos as u8);
            }
        }
    }
}
