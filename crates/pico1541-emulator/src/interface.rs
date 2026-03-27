
pub const MAX_TRACK_BYTES: usize = 10_000;

pub struct ActiveTrack {
    pub bit_length: u32,
    pub is_dirty: bool,
    pub disk_inserted: bool,
    pub gcr_data: [u8; MAX_TRACK_BYTES],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveEvent {
    LedChanged(bool),
    MotorChanged(bool),
    HeadStepped { new_half_track: u8 },
}

/// Represents the physical state of the IEC bus.
/// Bit 0: DATA
/// Bit 1: CLK
/// Bit 2: ATN
/// (1 = HIGH / 5V / Floating, 0 = LOW / 0V / Asserted)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IecBus(pub u8);

impl IecBus {
    // Bitmask constants
    pub const DATA: u8 = 0b001; // Bit 0
    pub const CLK:  u8 = 0b010; // Bit 1
    pub const ATN:  u8 = 0b100; // Bit 2

    // --- Fast Individual Getters ---
    #[inline(always)]
    pub fn data(self) -> bool { (self.0 & Self::DATA) != 0 }
    
    #[inline(always)]
    pub fn clk(self) -> bool { (self.0 & Self::CLK) != 0 }
    
    #[inline(always)]
    pub fn atn(self) -> bool { (self.0 & Self::ATN) != 0 }

    // --- Fast Individual Setters ---
    #[inline(always)]
    pub fn set_data(&mut self, state: bool) {
        if state { self.0 |= Self::DATA; } else { self.0 &= !Self::DATA; }
    }

    #[inline(always)]
    pub fn set_clk(&mut self, state: bool) {
        if state { self.0 |= Self::CLK; } else { self.0 &= !Self::CLK; }
    }

    #[inline(always)]
    pub fn set_atn(&mut self, state: bool) {
        if state { self.0 |= Self::ATN; } else { self.0 &= !Self::ATN; }
    }
}

pub trait HostInterface {
    /// Gets a temporary mutable reference to the currently active track.
    /// The Host resolves which of the two double-buffers this actually is.
    fn get_active_track(&mut self) -> &mut ActiveTrack;

    /// Send an event (LED, Motor, Step) to the Host
    fn send_event(&mut self, event: DriveEvent);

    /// IEC Bus pins
    fn read_iec(&self) -> IecBus;
    fn write_iec(&mut self, pins: IecBus);
}