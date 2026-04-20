use crate::via6522::Via6522;
use crate::interface::{DriveEvent, HostInterface};

// 35 tracks + 7 tracks more that can be physically reached (MAX 42)
const MAX_HALF_TRACK: u8 = (35 + 7) * 2; 

// Numer of CPU cycles until data is streamed to the VIA
const MOTOR_SPIN_UP_CYCLES: u32 = 250_000;
const MOTOR_SPIN_DOWN_THRESHOLD: u32 = 100_000;

// Mask used to determine the last state of stepper motor phases (PB0, PB1), MOTOR bit and ACT LED and (PB2, PB3)
const STATE_MASK: u8 = 0x0F;
const MOTOR_BIT: u8 = 0x04;
const LED_BIT: u8 = 0x08;
const STEP_BITS: u8 = 0x03;

pub struct DriveMechanics {
    pub current_half_track: u8,
    pub motor_spun_up: bool,
    pub motor_delay_counter: u32,
    pub current_byte_idx: usize,
    pub sub_cycle_counter: u32,

    last_state: u8,
}

impl DriveMechanics {
    pub fn new() -> Self {
        Self {
            current_half_track: 36, // Start at track 18
            motor_spun_up: false,
            motor_delay_counter: 0,
            current_byte_idx: 0,
            sub_cycle_counter: 0,
            last_state: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current_half_track = 36; // Start at track 18
        self.motor_spun_up = false;
        self.motor_delay_counter = 0;
        self.current_byte_idx = 0;
        self.sub_cycle_counter = 0;
        self.last_state = 0;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU.
    pub fn tick<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522) -> bool {
        // Update the drive state and issue events if there are changes
        let _track_changed = self.update_state(via2, hardware);

        // Was there a track change?
        // TODO! This needs some consideration. The half track change is not instant on real hardware, and similarly the
        //       Host will need time to load the new track after a step. We need to delegate the track change to the Host, 
        //       and once it is ready, we need to pick up the track number change, and realign the byte position.
        /*
        if step_direction != 0 {
            log::info!("HEAD Stepped: {} -> {}: {}, Track: {}",
                    self.last_step_phase, current_phase, step_direction, self.current_half_track);
            
            if self.current_half_track != old_half_track {
                // When stepping, the head moves but the disk keeps spinning.
                // Maintain the current rotational position by wrapping it to the new track length.
                let old_bit_length = {
                    let track = hardware.get_active_track();
                    track.bit_length
                };
                
                hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.current_half_track });
                
                let new_bit_length = {
                    let track = hardware.get_active_track();
                    track.bit_length
                };

                if old_bit_length > 0 && new_bit_length > 0 {
                    let old_bytes = (old_bit_length / 8) as usize;
                    let new_bytes = (new_bit_length / 8) as usize;
                    self.current_byte_idx = (self.current_byte_idx % old_bytes) % new_bytes;
                }
            }
        }

        self.last_step_phase = current_phase;
        

        // 2. Handle Motor and Rotation
        // On a 1541-II, the motor bit (VIA2 ORB bit 2) is active-LOW.
        let motor_on_target = (via2.orb & 0x04) == 0;

        if motor_on_target {
            if !self.motor_spun_up {
                self.motor_delay_counter += 1;
                if self.motor_delay_counter >= MOTOR_DELAY_CYCLES {
                    self.motor_spun_up = true;
                    self.motor_delay_counter = MOTOR_DELAY_CYCLES;
                    log::info!("Motor SPUN UP");
                }
            }
        } else {
            if self.motor_spun_up {
                self.motor_delay_counter = self.motor_delay_counter.saturating_sub(1);
                if self.motor_delay_counter == 0 {
                    self.motor_spun_up = false;
                    log::info!("Motor SPUN DOWN");
                }
            } else {
                self.motor_delay_counter = 0;
            }
        }*/

        // Fetch shared metadata — drop the reference before calling any other hardware methods
        let bit_length = {
            let track = hardware.get_active_track();
            if !track.disk_inserted || !self.motor_spun_up {
                return false;
            }
            track.bit_length
        };

        // 3. Bitrate and Disk Rotation
        let bitrate_bits = (via2.orb >> 5) & 0x03;
        let cycles_per_byte = match bitrate_bits {
            0 => 32, // Zone 0: Tracks 31-35
            1 => 30, // Zone 1: Tracks 25-30
            2 => 28, // Zone 2: Tracks 18-24
            3 => 26, // Zone 3: Tracks 1-17
            _ => 32,
        };

        self.sub_cycle_counter += 1;
        if self.sub_cycle_counter >= cycles_per_byte {
            self.sub_cycle_counter = 0;

            let track_bytes = (bit_length / 8) as usize;
            if track_bytes > 0 {
                self.current_byte_idx = (self.current_byte_idx + 1) % track_bytes;

                // VIA2 Port B bit 4: R/W mode — active-LOW (0 = write, 1 = read).
                // On real 1541-II hardware the R/W signal to the head amplifier
                // is active-LOW: the ROM clears PB4 to enable write, and leaves
                // it HIGH (=1) for normal read operation.
                let write_mode = (via2.orb & 0x10) == 0;

                if write_mode {
                    let track = hardware.get_active_track();
                    track.gcr_data[self.current_byte_idx] = via2.ora;
                    track.is_dirty = true;
                    return true;
                } else {
                    let gcr_byte = {
                        let track = hardware.get_active_track();
                        track.gcr_data[self.current_byte_idx]
                    };
                    via2.in_a = gcr_byte;
                    // Pulse CA1 to latch port A: on real hardware BYTE_READY fires
                    // both the 6502 SO pin and VIA2 CA1.
                    // Because Via6522::set_ca1 is public, we can use it to trigger
                    // the latching logic correctly.
                    via2.set_ca1(true);
                    via2.set_ca1(false);

                    if gcr_byte == 0xFF {
                        via2.in_b |= 0x80;  // SYNC found (1)
                        return false;       // No SO pulse for SYNC bytes
                    } else {
                        via2.in_b &= !0x80; // SYNC released (0)
                        if self.current_byte_idx % 1000 == 0 {
                            log::info!("GCR Byte: ${:02X} at idx {}", gcr_byte, self.current_byte_idx);
                        }
                        return true;        // SO pulse for data bytes
                    }
                }
            }
        }

        false
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self, via2: &Via6522) -> bool {
        (via2.orb & LED_BIT) != 0
    }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self, via2: &Via6522) -> bool {
        (via2.orb & MOTOR_BIT) != 0
    }

    /// Updates the drive mechanics state, and issues events accordingly.
    /// Returns true if the head position changed (stepper motor), false otherwise.
    #[inline(always)]
    fn update_state(&mut self, via2: &mut Via6522, hardware: &mut impl HostInterface) -> bool {
        // Catch state changes on the relevant bits of VIA2 Port B
        let curent_state = via2.orb & STATE_MASK;

        // Handle motor spin un/down logic immediately, as this is needed 
        // for each cycle and only depends on the current motor bit state.
        let motor_state = (curent_state & MOTOR_BIT) != 0;
        if motor_state && self.motor_delay_counter < MOTOR_SPIN_UP_CYCLES {
            self.motor_delay_counter += 1;
            if self.motor_delay_counter == MOTOR_SPIN_UP_CYCLES {
                log::info!("Motor SPUN UP");
                self.motor_spun_up = true;
            }             
        }
        else if !motor_state && self.motor_delay_counter > 0 {
            self.motor_delay_counter -= 1;
            if self.motor_spun_up && self.motor_delay_counter == MOTOR_SPIN_UP_CYCLES - MOTOR_SPIN_DOWN_THRESHOLD {
                log::info!("Motor SPUN DOWN");
                self.motor_spun_up = false;
            }
        }

        // Has anything changed?
        let changed = self.last_state ^ curent_state;
        if changed == 0 {
            return false; // No relevant state changes
        }

        // Keep the phase bits of the last state, and update the last state
        let last_phase = self.last_state & STEP_BITS;
        self.last_state = curent_state;

        // Handle LED changes (VIA2 Port B bit 3)
        if (changed & LED_BIT) != 0 {
            let led_state = (curent_state & LED_BIT) != 0;
            log::info!("LED changed: {}", led_state);
            hardware.send_event(DriveEvent::LedChanged(led_state));
        }

        // Handle Motor changes (VIA2 Port B bit 2)
        if (changed & MOTOR_BIT) != 0 {
            // NOTE! We send the event immediately!
            log::info!("Motor changed: {}", motor_state);
            hardware.send_event(DriveEvent::MotorChanged(motor_state));
        }

        // Handle Stepper changes (VIA2 Port B bits 0 and 1)
        if (changed & STEP_BITS) != 0 {
            let new_phase = curent_state & STEP_BITS;

            // Determine the step direction based on the phase change
            // Anta at 'old_bits' er forrige verdi (0-3) og 'new_bits' er nåværende (0-3)
            let delta = (new_phase.wrapping_sub(last_phase) & 3) as i8;
            let step = match delta {
                1 => 1,  // Step in
                3 => -1, // Step out
                _ => 0,  // No change or invalid transition (e.g., 0->2 or 1->3)
            };

            // Ignore invalid transitions that don't correspond to a single step
            if step == 0 {
                log::warn!("Invalid step detected: last_phase={}, new_phase={}", last_phase, new_phase);
                return false;
            }

            // Update the current half track, ensuring it stays within valid bounds
            self.current_half_track = (self.current_half_track as i8 + step)
                    .clamp(0, MAX_HALF_TRACK as i8) as u8;

            log::info!("HEAD Stepped: Phase {} -> {} ({}), Half-Track: {}", last_phase, new_phase, step, self.current_half_track);
            hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.current_half_track });

            return true;
        }

        // No track change
        false
    } 
        
}
