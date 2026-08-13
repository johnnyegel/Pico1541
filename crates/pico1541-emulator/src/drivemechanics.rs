use pico1541_common::track_buffer::{EmulatorHandle, TrackIdentifier, TrackBuffer};
use pico1541_common::{magnetic_surface::MagneticSurface, interface::HostInterface};

pub struct ReadHeadSignals {
    pub byte_opt: Option<u8>,
    pub is_sync: bool,
}

#[cfg(feature = "debug")]
use crate::via6522::Via6522;

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
const MASK_STEP: u8 = 0x03;
#[cfg(feature = "debug")]
const MASK_LED: u8 = 0x08;

// Mask used to determine the last state of stepper motor phases (PB0, PB1), MOTOR bit and ACT LED and (PB2, PB3)
const MASK_HARDWARE_STATE: u8 = 0x0F;
const MASK_SPEED_ZONE: u8 = 0x60;

const SHIFT_SPEED_ZONE: u8 = 5;

/// DriveMechanics struct to represent the state of the drive's mechanical components, such as the head position and motor state.
pub struct DriveMechanics<S: MagneticSurface> {
    /// The Track the head is positioned over (not necessarily the track being read)
    pub halftrack_head_pos: i8,
    
    pub surface: S,
    pub track_handle: EmulatorHandle,
    pub current_identifier: TrackIdentifier,
    
    last_state: u8,
    last_requested_sector: u8,
    last_active_index: u8,
}

impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn new(surface: S, track_handle: EmulatorHandle) -> Self {
        Self {
            halftrack_head_pos: INITIAL_HALF_TRACK,
            surface,
            track_handle,
            current_identifier: TrackIdentifier::void(),
            last_state: 0,
            last_requested_sector: 255,
            last_active_index: 0,
        }
    }
}

impl<S: MagneticSurface> DriveMechanics<S> {
    pub fn reset(&mut self) {
        self.halftrack_head_pos = INITIAL_HALF_TRACK;
        self.current_identifier = TrackIdentifier::void();
        self.last_state = 0;
        self.last_requested_sector = 0xFF;
    }

    pub fn is_write_protected(&self) -> bool {
        self.track_handle.is_write_protected()
    }

    pub fn pop_force_disk_change(&mut self) -> bool {
        self.track_handle.pop_force_disk_change()
    }
    
    pub fn write_byte_at(&mut self, index: u32, data: u8) {
        self.track_handle.seek_to_byte(index);
        let expected_identifier = self.current_identifier;
        self.track_handle.next_write_byte(expected_identifier, 0xFF, data);
    }

    /// Updates the drive mechanics state based on VIA2 Port B outputs.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn update_state<H: HostInterface>(&mut self, effective_orb: u8, hardware: &mut H, master_clock: u32) {
        let full_state = effective_orb & (MASK_HARDWARE_STATE | MASK_SPEED_ZONE);
        if full_state != self.last_state {
            self.update_state_slow(full_state, hardware, master_clock);
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn half_track(&self) -> u8 {
        // Do not expose the physical half-track outside the drive mechanics.
        self.halftrack_head_pos.clamp(0, MAX_HALF_TRACK) as u8
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn track_byte_pos_ix(&self) -> usize {
        self.surface.get_absolute_head_position(0) as usize
    }

    #[cfg(feature = "debug")]
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn led_on(&self, via2: &Via6522) -> bool {
        (via2.get_pb_out() & MASK_LED) != 0
    }

    #[cfg(feature = "debug")]
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn motor_on(&self, via2: &Via6522) -> bool {
        ((via2.orb | !via2.ddrb) & MASK_MOTOR) != 0
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn advance_read_head(&mut self, delta: u32, master_clock: u32) -> ReadHeadSignals {
        self.check_active_index(master_clock);
        ReadHeadSignals {
            byte_opt: self.surface.try_read_byte(delta),
            is_sync: self.surface.is_sync_active(),
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn advance_write_head(&mut self, delta: u32, master_clock: u32, byte: u8) {
        self.check_active_index(master_clock);
        // Writing ignores delta for time advancement because the surface time is driven by it.
        // Wait, the surface needs to advance by delta!
        // try_read_byte advances time. The write mode just advances time by `delta` cycles but doesn't output bytes.
        // Actually, try_read_byte is the only way to advance the surface time right now.
        // Let's use try_read_byte to advance time and just ignore the output byte.
        let _ = self.surface.try_read_byte(delta);
        let index = self.surface.get_absolute_head_position(0);
        self.write_byte_at(index, byte);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn check_active_index(&mut self, master_clock: u32) {
        let current_active_index = self.track_handle.get_generation();
        if current_active_index != self.last_active_index {
            log::info!("TICK SEES CHANGE! current={} last={}", current_active_index, self.last_active_index);
            self.last_active_index = current_active_index;
            self.evaluate_track_source(master_clock);
        }
    }

    // ========== Internal helper functions ==================================================

    
    pub fn force_state_for_testing(&mut self, halftrack: u8, speed_zone: u8, master_clock: u32) {
        let new_track_identifier = TrackIdentifier::new(halftrack, speed_zone);
        self.halftrack_head_pos = halftrack as i8;
        self.last_state = (self.last_state & !MASK_SPEED_ZONE) | (speed_zone << SHIFT_SPEED_ZONE);
        self.track_handle.update_head_half_track(halftrack);
        self.surface.set_speed_zone(speed_zone);
        self.evaluate_track_identifier(new_track_identifier, master_clock);
    }

    /// Updates the drive mechanics state, and issues events accordingly.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn update_state_slow<H: HostInterface>(&mut self, full_state: u8, hardware: &mut H, master_clock: u32) {
        let last_state = self.last_state;
        let state_changes = full_state ^ last_state;
        self.last_state = full_state;

        if (state_changes & MASK_MOTOR) != 0 {
            let motor_on = (full_state & MASK_MOTOR) != 0;
            log::info!("Motor state changed: motor_on = {}, head_pos = {}", motor_on, self.halftrack_head_pos);
            self.surface.set_motor_running(motor_on, master_clock);
        }

        // :: Update stepper motor and tracking
        // Did speed-zone change? (PB5, PB6)
        if (state_changes & MASK_SPEED_ZONE) != 0 {
            let speed_zone = (full_state & MASK_SPEED_ZONE) >> SHIFT_SPEED_ZONE;
            let new_track_identifier = self.current_identifier.with_speed_zone(speed_zone);
            self.surface.set_speed_zone(speed_zone);
            self.evaluate_track_identifier(new_track_identifier, master_clock);
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

            if step != 0 {
                self.halftrack_head_pos = (last_half_track_pos + step)
                        .clamp(MIN_HALF_TRACK, MAX_HALF_TRACK);

                if self.halftrack_head_pos >= 0 && self.halftrack_head_pos <= MAX_HALF_TRACK {
                    let current_half_track = self.halftrack_head_pos as u8;
                    let new_track_identifier = self.current_identifier.with_half_track(current_half_track);
                    self.track_handle.update_head_half_track(current_half_track);
                    self.evaluate_track_identifier(new_track_identifier, master_clock);
                    
                    hardware.track_changed(current_half_track);
                }
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn evaluate_track_identifier(&mut self, new_track_identifier: TrackIdentifier, master_clock: u32) {
        // :? Has identifier changed?
        if self.current_identifier != new_track_identifier {
            // -> Yes, then we need to update the track source.
            self.current_identifier = new_track_identifier;
            self.evaluate_track_source(master_clock);
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn evaluate_track_source(&mut self, master_clock: u32) {
        let loaded_identifier = self.track_handle.get_active_track_identifier();
        
        log::info!("EVAL current={:?} loaded={:?}", self.current_identifier, loaded_identifier);
        
        if self.current_identifier == loaded_identifier {
            let length = self.track_handle.get_active_track_length() as u32;
            if length > 0 {
                let stream = self.track_handle.get_active_track_buffer();
                self.surface.set_stream_source(stream, master_clock);
            } else {
                self.surface.set_stream_source(TrackBuffer::noise(), master_clock);
            }
        } else {
            self.surface.set_stream_source(TrackBuffer::noise(), master_clock);
        }
    }

}
