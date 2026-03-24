use core::sync::atomic::{ AtomicU8, AtomicBool };

/// Maximum size of a single 1541 track in raw GCR bytes.
/// Standard tracks are ~8KB, but copy-protected tracks can be pushed to ~9.5KB.
pub const MAX_TRACK_BYTES: usize = 10_000;

/// Represents the physical state of the currently loaded disk track.
pub struct ActiveTrack {
    /// The physical half-track number (e.g., Track 1.0 = 2, Track 18.5 = 37).
    /// Halftracks are heavily used in copy protection.
    pub half_track_id: AtomicU8,
    
    /// True if a disk is physically inserted in the drive
    pub disk_inserted: AtomicBool,

    /// True if Core 0 has written data to this track, signaling Core 1 
    /// that it needs to save the flux changes back to the USB/SD.
    pub is_dirty: AtomicBool,

    /// Exact number of bits on this track. This varies depending on the speed zone
    /// (outer tracks hold more data than inner tracks) and copy protection anomalies.
    pub bit_length: u32,

    /// The unified GCR bitstream. 
    /// If a D64 was loaded, Core 1 generated the GCR sync marks and gaps to fill this.
    /// If a G64 was loaded, Core 1 copied the raw flux bytes directly here.
    pub gcr_data: [u8; MAX_TRACK_BYTES],
}

impl ActiveTrack {
    pub const fn new() -> Self {
        Self {
            half_track_id: AtomicU8::new(0),
            disk_inserted: AtomicBool::new(false),
            is_dirty: AtomicBool::new(false),
            bit_length: 0,
            gcr_data: [0; MAX_TRACK_BYTES],
        }
    }
}