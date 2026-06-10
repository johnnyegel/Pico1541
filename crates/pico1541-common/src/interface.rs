use core::fmt;
use core::fmt::Display;




/// Represents the bus state of the IEC bus lines
/// Bit 0: DATA, Bit 1: CLK, Bit 2: ATN
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IecBusState(pub u8);

impl IecBusState {
    pub const DATA_BIT: u8 = 0b001;
    pub const CLK_BIT: u8 = 0b010;
    pub const ATN_BIT: u8 = 0b100;

    pub fn from_bits(atn: bool, clk: bool, data: bool) -> Self {
        let mut bits = 0;
        if atn { bits |= Self::ATN_BIT; }
        if clk { bits |= Self::CLK_BIT; }
        if data { bits |= Self::DATA_BIT; }
        Self(bits)
    }

    /// Returns true if DATA line is asserted
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_data(&self) -> bool {
        self.0 & 0b001 != 0
    }
    /// Returns true if CLK line is asserted
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_clk(&self) -> bool {
        self.0 & 0b010 != 0
    }
    /// Returns true if ATN line is asserted
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn get_atn(&self) -> bool {
        self.0 & 0b100 != 0
    }

    // Set the data bit
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn set_data(&mut self, asserted: bool) {
        if asserted {
            self.0 |= Self::DATA_BIT;
        } else {
            self.0 &= !Self::DATA_BIT;
        }
    }

    /// Set the clk bit
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn set_clk(&mut self, asserted: bool) {
         if asserted {
            self.0 |= Self::CLK_BIT;
        } else {
            self.0 &= !Self::CLK_BIT;
        }
    }

    // Set the atn bit
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn set_atn(&mut self, asserted: bool) {
         if asserted {
            self.0 |= Self::ATN_BIT;
        } else {
            self.0 &= !Self::ATN_BIT;
        }
    }

    /// ORs together the current state with another IecBusState, 
    /// used for combining host and drive assertions.
    /// NOTE! This is only needed in Unit tests, as the hardware in the emulator 
    /// will already provide the combined state to the Pico input pins.
    pub fn combine(&self, other: &IecBusState) -> IecBusState {
        IecBusState(self.0 | other.0)
    }

    /// Returns the difference between this state and the last state resuling in any edge changes being asserted.
    /// Returns a bitmask indicating which lines have changed (aka edge detection)
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn delta(&self, last_state: &IecBusState) -> IecBusState {
        IecBusState(self.0 ^ last_state.0)
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
        drive_track: u8,
        drive_sector: u8,
    },
    /// The Snooper has detected a secondary address byte addressed to this device
    IecSnoopSecondaryAddress(u8), // secondary address byte
    /// Byte received to this device, on the previous secondary address
    IecSnoopByteReceived(u8, bool), // byte, is_eoi
    /// A complete decoded IEC command (e.g. "CD:...")
    IecCommand {
        data: [u8; 32],
        len: u8,
    },
    /// The Snooper detected a CLOSE command for a specific secondary address
    IecCloseChannel(u8),
    /// The Snooper has detected the end of a transaction (e.g. UNLISTEN or ATN assertion)
    IecSnoopEndOfTransaction,
    IecSnoopTransactionComplete,
    /// The Snooper detected that the IEC bus has been fully idle (all lines released) for > 250ms.
    IecBusIdleTimeout,
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
    fn sync(&mut self, _host: IecBusState) {}

    /// Called every emulator tick cycle, independent of bus changes.
    fn step(&mut self) {}

    /// Update the IEC addresses (LISTEN and TALK) snooped by the manager.
    /// Usually called by the emulator when it detects a change in drive RAM.
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) { }


    fn get_device_id(&self) -> u8 {
        8
    }

    /// Returns the current PC of the CPU (for debugging)
    fn get_pc(&self) -> u16 {
        0
    }
}
