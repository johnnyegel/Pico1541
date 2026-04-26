
pub const MAX_TRACK_BYTES: usize = 10_000;

pub struct ActiveTrack {
    pub bit_length: u32,
    pub current_byte_idx: u32,
    pub is_dirty: bool,
    pub disk_inserted: bool,
    /// True if the track contains real GCR data. False for unformatted/missing
    /// half-tracks (e.g. D64 images have no half-track data): the head reads
    /// back a continuous stream of 0x55 bytes with no SYNC.
    pub is_formatted: bool,
    pub gcr_data: [u8; MAX_TRACK_BYTES],
}

impl ActiveTrack {
    pub fn new() -> Self {
        Self {
            bit_length: 0,
            current_byte_idx: 0,
            is_dirty: false,
            disk_inserted: false,
            is_formatted: true,
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
    /// A snooped IEC command directed at channel 15
    IecCommand {
        data: [u8; 64],
        len: u8,
    },
}

/// Open-drain IEC serial bus interface.
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
    /// Returns true if the host is asserting DATA.
    fn read_data(&self) -> bool;
    /// Returns true if the host is asserting CLK.
    fn read_clk(&self) -> bool;
    /// Returns true if the host is asserting ATN.
    fn read_atn(&self) -> bool;

    /// Called each cycle with the drive output DATA bus state (true = asserted).
    fn drive_data(&mut self, assert: bool);
    /// Called each cycle with the drive output CLK bus state (true = asserted).
    fn drive_clk(&mut self, assert: bool);
}

pub trait HostInterface: IecBus {
    /// Gets a temporary mutable reference to the currently active track.
    /// The Host resolves which of the two double-buffers this actually is.
    fn get_active_track(&mut self) -> &mut ActiveTrack;

    /// Send an event (LED, Motor, Step) to the Host
    fn send_event(&mut self, event: DriveEvent);

    /// Returns true if the bus is currently hijacked by the manager.
    fn is_hijacked(&self) -> bool { false }

    /// Returns true if the manager wants to discard the ROM command buffer.
    /// This should clear the request after returning true.
    fn pop_discard_request(&mut self) -> bool { false }

    /// Synchronization point called every cycle.
    fn sync(&mut self) {}

    /// Update the IEC addresses (LISTEN and TALK) snooped by the manager.
    /// Usually called by the emulator when it detects a change in drive RAM.
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}
}