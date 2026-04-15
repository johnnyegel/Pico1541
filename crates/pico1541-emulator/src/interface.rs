
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

/// Open-drain IEC serial bus interface.
///
/// `read_*` methods return the current state of each line as seen from the bus
/// (true = asserted/LOW, pulled by the host/C64 side).  These must NOT reflect
/// the drive's own outputs — the input path is isolated on real hardware to
/// prevent feedback loops in the 1541 ROM.
///
/// `drive_*` methods let the emulator assert or release each line.  On real
/// hardware this toggles a GPIO PINDIR; in simulation it updates the combined
/// open-drain state that the host makes visible.
pub trait IecBus {
    /// Returns true if the line is asserted (pulled LOW by the host)
    fn read_data(&self) -> bool;
    fn read_clk(&self) -> bool;
    fn read_atn(&self) -> bool;

    /// Asserts (true) or releases (false) the line
    fn drive_data(&mut self, assert: bool);
    fn drive_clk(&mut self, assert: bool);
}

pub trait HostInterface: IecBus {
    /// Gets a temporary mutable reference to the currently active track.
    /// The Host resolves which of the two double-buffers this actually is.
    fn get_active_track(&mut self) -> &mut ActiveTrack;

    /// Send an event (LED, Motor, Step) to the Host
    fn send_event(&mut self, event: DriveEvent);
}