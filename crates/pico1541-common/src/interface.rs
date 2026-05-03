use core::fmt;
use core::fmt::Display;

pub const MAX_TRACK_BYTES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct ActiveTrack {
    pub half_track: u8,
    pub bit_length: u32,
    pub current_byte_idx: u32,
    pub is_dirty: bool,
    pub disk_inserted: bool,
    /// True if the track contains real GCR data. False for unformatted/missing
    /// half-tracks (e.g. D64 images have no half-track data): the head reads
    /// back a continuous stream of 0x55 bytes with no SYNC.
    pub is_formatted: bool,
    /// If true, the drive emulates real physical rotation.
    /// If false, the drive can "warp" to the next sector immediately.
    pub is_accurate: bool,
    pub gcr_data: [u8; MAX_TRACK_BYTES],
}

impl ActiveTrack {
    pub fn new() -> Self {
        Self {
            half_track: 0,
            bit_length: 0,
            current_byte_idx: 0,
            is_dirty: false,
            disk_inserted: false,
            is_formatted: true,
            is_accurate: true,
            gcr_data: [0; MAX_TRACK_BYTES],
        }
    }
}

impl Default for ActiveTrack {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents the bus state of the IEC bus lines
/// Bit 0: DATA, Bit 1: CLK, Bit 2: ATN
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IecBusState(pub u8);

impl IecBusState {
    pub const DATA_BIT: u8 = 0b001;
    pub const CLK_BIT: u8 = 0b010;
    pub const ATN_BIT: u8 = 0b100;

    /// Returns true if DATA line is asserted
    pub fn get_data(&self) -> bool {
        self.0 & 0b001 != 0
    }
    /// Returns true if CLK line is asserted
    pub fn get_clk(&self) -> bool {
        self.0 & 0b010 != 0
    }
    /// Returns true if ATN line is asserted
    pub fn get_atn(&self) -> bool {
        self.0 & 0b100 != 0
    }

    // Set the data bit
    pub fn set_data(&mut self, asserted: bool) {
        if asserted {
            self.0 |= Self::DATA_BIT;
        } else {
            self.0 &= !Self::DATA_BIT;
        }
    }

    /// Set the clk bit
    pub fn set_clk(&mut self, asserted: bool) {
         if asserted {
            self.0 |= Self::CLK_BIT;
        } else {
            self.0 &= !Self::CLK_BIT;
        }
    }

    // Set the atn bit
    pub fn set_atn(&mut self, asserted: bool) {
         if asserted {
            self.0 |= Self::ATN_BIT;
        } else {
            self.0 &= !Self::ATN_BIT;
        }
    }

    /// Returns the difference between this state and the last state
    /// Returns a bitmask indicating which lines have changed (aka edge detection)
    pub fn delta(&self, last_state: &IecBusState) -> u8 {
        self.0 ^ last_state.0
    }
}

impl Default for IecBusState {
    fn default() -> Self {
        Self(0)
    }   
}

impl Display for IecBusState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IecBusState({:#02X}: ATN={}, CLK={}, DATA={}}}",
            self.0,
            self.get_atn(),
            self.get_clk(),
            self.get_data())
    }
}

/// Events issues by the drive emulator
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveEvent {
    LedChanged(bool),
    MotorChanged(bool),
    HeadStepped {
        new_half_track: u8,
    },
    /// A snooped IEC command directed at channel 15
    IecCommand {
        data: [u8; 64],
        len: u8,
    },
    /// Low-level IEC bus state change (State Byte, Delta Cycles)
    IecBusTransition(crate::snooper::IecBusByte, u32),
}

/// Open-drain IEC serial bus interface (Emulator side).
///
/// All signals use the logical convention: **true = asserted**.
/// On the physical IEC bus an asserted line is pulled LOW; this inversion is a
/// hardware concern and is NOT reflected here.  The emulator works entirely in
/// the logical domain — no caller needs to invert values.
///
/// `read_*` methods return what the host (C64 side) is currently driving.
/// They must NOT reflect the drive's own outputs; the 1541's input path is
/// isolated from its output path on real hardware.
///
/// `drive_data` / `drive_clk` are called by the emulator each CPU cycle with
/// the **combined open-drain bus state**: host assertion OR drive assertion OR
/// ATNA contribution.  true = the line is asserted by at least one party.
pub trait IecBus {
    /// Reads the current bus state from the host as a bitmask
    /// See `IecBusState` for bit definitions.
    fn iec_host(&self) -> IecBusState;

    /// Called each cycle with the combined open-drain bus state (true = asserted).
    fn iec_drive(&mut self, state: IecBusState);
}

pub trait HostInterface: IecBus {
    /// Gets a temporary mutable reference to the currently active track.
    /// The Host resolves which of the two double-buffers this actually is.
    fn get_active_track(&mut self) -> &mut ActiveTrack;

    /// Send an event (LED, Motor, Step) to the Host
    fn send_event(&mut self, event: DriveEvent);

    /// Returns true if the bus is currently hijacked by the manager.
    fn is_hijacked(&self) -> bool {
        false
    }

    /// Returns true if the manager wants to discard the ROM command buffer.
    /// This should clear the request after returning true.
    fn pop_discard_request(&mut self) -> bool {
        false
    }

    /// synchronization point called every cycle.
    fn sync(&mut self) {}

    /// Update the IEC addresses (LISTEN and TALK) snooped by the manager.
    /// Usually called by the emulator when it detects a change in drive RAM.
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}
}
