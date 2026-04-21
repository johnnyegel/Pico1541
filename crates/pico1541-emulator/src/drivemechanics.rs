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
    initialized: bool,
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
            initialized: false,
        }
    }

    pub fn reset(&mut self) {
        self.current_half_track = 36; // Start at track 18
        self.motor_spun_up = false;
        self.motor_delay_counter = 0;
        self.current_byte_idx = 0;
        self.sub_cycle_counter = 0;
        self.last_state = 0;
        self.initialized = false;
    }

    /// Progresses the drive mechanics by one CPU cycle.
    /// Returns true if a new byte is ready to be processed by the CPU.
    pub fn tick<H: HostInterface>(&mut self, hardware: &mut H, via2: &mut Via6522) -> bool {
        // On the very first tick, seed only the bitrate bits so that a zone change
        // on tick-1 is not falsely reported. Motor/LED/step bits are left at 0 so
        // the first ROM write that turns the motor on is detected as a real transition.
        if !self.initialized {
            self.last_state = via2.orb & 0x60;
            self.initialized = true;
        }

        // Update the drive state and issue events if there are changes
        let changed = self.update_state(via2, hardware);

        // Reset sub_cycle_counter if bitrate changes (PB5, PB6)
        // We do this BEFORE incrementing it below.
        if (changed & 0x60) != 0 {
            self.sub_cycle_counter = 0;
        }

        // Bitrate zone (VIA2 PB5/PB6) determines cycles-per-byte for all paths below.
        let bitrate_bits = (via2.orb >> 5) & 0x03;
        let cycles_per_byte: u32 = match bitrate_bits {
            0 => 32, // Zone 0: Tracks 31-35
            1 => 30, // Zone 1: Tracks 25-30
            2 => 28, // Zone 2: Tracks 18-24
            3 => 26, // Zone 3: Tracks 1-17
            _ => 32,
        };

        // Fetch shared metadata — drop the reference before calling any other hardware methods
        let (bit_length, is_formatted) = {
            let track = hardware.get_active_track();
            if !self.motor_spun_up {
                self.sub_cycle_counter = 0;
                self.current_byte_idx = 0;
                via2.in_b &= !0x80;
                return false;
            }
            if !track.disk_inserted {
                // Motor running but no disk: deliver 0x55 noise bytes at the normal bitrate.
                // Without this, the ROM sits in a tight `CLV / BVC` loop waiting for the SO
                // pin (byte-ready), which is never asserted when no bytes arrive, hanging the
                // CPU forever instead of reaching the ROM's own sync-search timeout.
                via2.in_b &= !0x80; // SYNC never asserted — no disk
                self.sub_cycle_counter += 1;
                if self.sub_cycle_counter >= cycles_per_byte {
                    self.sub_cycle_counter = 0;
                    via2.in_a = 0x55;
                    via2.set_ca1(true);
                    via2.set_ca1(false);
                    return true; // pulse SO
                }
                return false;
            }
            (track.bit_length, track.is_formatted)
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
                    let gcr_byte = if !is_formatted {
                        // Unformatted/missing track — head reads back flux noise,
                        // represented as 0x55. No SYNC is ever produced.
                        0x55
                    } else {
                        let track = hardware.get_active_track();
                        track.gcr_data[self.current_byte_idx]
                    };

                    // Port A is always updated with the byte from the head,
                    // even if it is SYNC (which is then ignored by the CPU
                    // because SYNC is asserted).
                    via2.in_a = gcr_byte;

                    if is_formatted && gcr_byte == 0xFF {
                        via2.in_b |= 0x80;  // SYNC found (1)
                        // SYNC bytes do not trigger a BYTE READY (SO/CA1) pulse.
                        return false;
                    } else {
                        via2.in_b &= !0x80; // SYNC released (0)

                        // Pulse CA1 to latch port A: on real hardware BYTE_READY fires
                        // both the 6502 SO pin and VIA2 CA1.
                        via2.set_ca1(true);
                        via2.set_ca1(false);

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
    /// Returns the bits that changed on the relevant Port B mask.
    #[inline(always)]
    fn update_state(&mut self, via2: &mut Via6522, hardware: &mut impl HostInterface) -> u8 {
        // Catch state changes on the relevant bits of VIA2 Port B (including bitrate)
        let full_relevant_state = via2.orb & (STATE_MASK | 0x60);
        let curent_state = full_relevant_state & STATE_MASK;

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
        let changed = self.last_state ^ full_relevant_state;
        if changed == 0 {
            return 0; // No relevant state changes
        }

        // Keep the phase bits of the last state, and update the last state
        let last_phase = self.last_state & STEP_BITS;
        self.last_state = full_relevant_state;

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
            if step != 0 {
                // Update the current half track, ensuring it stays within valid bounds
                self.current_half_track = (self.current_half_track as i8 + step)
                        .clamp(0, MAX_HALF_TRACK as i8) as u8;

                log::info!("HEAD Stepped: Phase {} -> {} ({}), Half-Track: {}", last_phase, new_phase, step, self.current_half_track);
                hardware.send_event(DriveEvent::HeadStepped { new_half_track: self.current_half_track });
            } else {
                log::warn!("Invalid step detected: last_phase={}, new_phase={}", last_phase, new_phase);
            }
        }

        // Return the full change mask
        changed
    } 
        
}
