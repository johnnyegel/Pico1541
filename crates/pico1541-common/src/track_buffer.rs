/// Implements a double track buffer for loading data from images
/// and handing over to the drive emulator. This works by splitting 
/// a double buffer into two handles, and exchanging the buffers
/// between the Manager and Emulator

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

// Private constants for internal use
const TRACK_SIZE: usize = 8192; // Maximum GCR-encoded track size in
const VOID_BYTE: u8 = 0x55; // Byte value for void tracks


#[derive(Clone, Copy, PartialEq)]
pub enum TrackLayout {
    /// An unformatted track with no data. Reads 0x55 for all bytes, and can not be written to.
    /// Used to indicate there is no disk present in the drive, or the track is not loaded.
    Void,
    /// A constructed track where the byte position can be warped to any position instantly.
    Warped(usize),
    /// An accurately emulated track where the byte position on track change is proportional
    /// to the track size and position on track change.
    Accurate(usize),
}

/// Represents a single track buffer which holds the GCR-encoded data for a track,
/// along with metadata about the track such as its ID and layout.
pub struct TrackBuffer {
    pub data: [u8; 8192],
    pub track_id: u8,       // The half-track index (0-83)
    pub length: usize,      // Valid bytes in the buffer
    pub layout: TrackLayout,
}

impl TrackBuffer {
    const fn new() -> Self {
        Self {
            data: [0; 8192],
            track_id: 0xFF, // 0xFF represents invalid/unloaded
            length: 0,
            layout: TrackLayout::Void,
        }
    }
}

/// Dual core track buffer that can be split into two handles for the Emulator and Manager
pub struct DualCoreTrackBuffer {
    // Now holding our unified struct
    buffers: UnsafeCell<[TrackBuffer; 2]>,
    active_index: AtomicBool,
    dirty_flags: [AtomicBool; 2],
}

impl DualCoreTrackBuffer {
    pub const fn new() -> Self {
        Self {
            buffers: UnsafeCell::new([TrackBuffer::new(), TrackBuffer::new()]),
            active_index: AtomicBool::new(false),
            dirty_flags: [AtomicBool::new(false), AtomicBool::new(false)],
        }
    }

    /// Consumes the reference and returns two distinct halves.
    pub fn split(&'static self) -> (ManagerHandle, EmulatorHandle) {
        (
            ManagerHandle { core_buffer: self },
            EmulatorHandle { core_buffer: self },
        )
    }
}

// =====================================================================
// The MANAGER handle
// =====================================================================
pub struct ManagerHandle {
    core_buffer: &'static DualCoreTrackBuffer,
}

impl ManagerHandle {
    pub fn load_next_track<F>(&self, fill_buffer: F)
    where
        F: FnOnce(&mut [u8; TRACK_SIZE]),
    {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let back_idx = if current_active { 0 } else { 1 };

        // SAFELY isolate the mutable reference to the dormant buffer.
        let back_buffer = unsafe { 
            &mut (*self.core_buffer.buffers.get())[back_idx] 
        };

        fill_buffer(&mut back_buffer.data);
        self.core_buffer.active_index.store(!current_active, Ordering::Release);
    }
}

// =====================================================================
// The EMULATOR handle
// =====================================================================
pub struct EmulatorHandle {
    core_buffer: &'static DualCoreTrackBuffer,
}

impl EmulatorHandle {
    // Safely reads a GCR byte. 
    /// Automatically handles the modulo math and returns 0x55 noise 
    /// if the head is in transit or unformatted.
    #[inline(always)]
    pub fn read_gcr_byte(&self, expected_track_id: u8, byte_index: usize) -> u8 {
        // 1. Lock-free active buffer resolution
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };

        // SAFETY: We only dereference the active index. 
        // Core 0 guarantees it only ever mutates the dormant index.
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };

        // 2. The Transit / Noise Filter
        if buffer.track_id != expected_track_id || buffer.length == 0 {
            return 0x55;
        }

        // 3. Safe, modulo-wrapped array access
        buffer.data[byte_index % buffer.length]
    }

    /// Safely writes a GCR byte back to the disk buffer and flags it for background SD flush.
    #[inline(always)]
    pub fn write_gcr_byte(&self, expected_track_id: u8, byte_index: usize, data: u8) {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        
        let buffer = unsafe { &mut (*self.core_buffer.buffers.get())[active_idx] };

        // Only allow writes if the head is actually settled on the expected track
        if buffer.track_id == expected_track_id && buffer.length > 0 {
            buffer.data[byte_index % buffer.length] = data;
            
            // Mark this specific buffer as dirty so Core 0 knows to flush it later
            self.core_buffer.dirty_flags[active_idx].store(true, Ordering::Release);
        }
    }

    /// Fetches the track layout to calculate bit-rates (Speed Zones).
    /// Returns None if the track is in transit.
    #[inline(always)]
    pub fn get_layout(&self, expected_track_id: u8) -> Option<TrackLayout> {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };

        if buffer.track_id == expected_track_id {
            Some(buffer.layout)
        } else {
            None
        }
    }
}

