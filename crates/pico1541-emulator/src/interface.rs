
pub const MAX_TRACK_BYTES: usize = 10_000;

pub struct ActiveTrack {
    pub bit_length: u32,
    pub is_dirty: bool,
    pub disk_inserted: bool,
    pub gcr_data: [u8; MAX_TRACK_BYTES],
}

impl ActiveTrack {
    pub fn new() -> Self {
        Self {
            bit_length: 0,
            is_dirty: false,
            disk_inserted: false,
            gcr_data: [0; MAX_TRACK_BYTES],
        }
    }
}

impl Default for ActiveTrack {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveEvent {
    LedChanged(bool),
    MotorChanged(bool),
    HeadStepped { new_half_track: u8 },
}

/// Represents the physical state of the IEC bus (open-drain asserted convention).
/// Bit 0: DATA  (1 = LOW/asserted/pulled-to-ground, 0 = HIGH/released/floating)
/// Bit 1: CLK   (1 = LOW/asserted, 0 = HIGH/released)
/// Bit 2: ATN   (1 = LOW/asserted, 0 = HIGH/released)
/// IecBus(0x00) = all lines released = idle. Open-drain OR: bus = host | drive.
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