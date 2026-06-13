/// Implements a double track buffer for loading data from images
/// and handing over to the drive emulator. This works by splitting 
/// a double buffer into two handles, and exchanging the buffers
/// between the Manager and Emulator

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

// Private constants for internal use
const TRACK_SIZE: usize = 8192; // Maximum GCR-encoded track size in
const VOID_BYTE: u8 = 0x55; // Byte value for void tracks


#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TrackLayout {
    /// An unformatted track with no data. Reads 0x55 for all bytes, and can not be written to.
    /// Used to indicate there is no disk present in the drive, or the track is not loaded.
    Void,
    /// A constructed track where the byte position can be warped to any position instantly.
    /// The usize represents the sector stride.
    Warped(usize),
    /// An accurately emulated track where the byte position on track change is proportional
    /// to the track size and position on track change.
    Accurate,
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
    pub is_write_protected: AtomicBool,
    pub force_disk_change: AtomicBool,
    pub current_byte_idx: core::sync::atomic::AtomicUsize,
}

unsafe impl Sync for DualCoreTrackBuffer {}
unsafe impl Send for DualCoreTrackBuffer {}

impl DualCoreTrackBuffer {
    pub const fn new() -> Self {
        Self {
            buffers: UnsafeCell::new([TrackBuffer::new(), TrackBuffer::new()]),
            active_index: AtomicBool::new(false),
            is_write_protected: AtomicBool::new(false),
            force_disk_change: AtomicBool::new(false),
            current_byte_idx: core::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Consumes the reference and returns two distinct halves.
    pub fn split(&'static self) -> (ManagerHandle, EmulatorHandle) {
        (
            ManagerHandle { core_buffer: self },
            EmulatorHandle { 
                core_buffer: self,
                track_byte_pos_ix: 0,
                track_byte_size: 0,
                last_requested_sector: 0xFF,
                last_is_listening: false,
            },
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
        F: FnOnce(&mut [u8; TRACK_SIZE], &mut TrackLayout, &mut u8),
    {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let back_idx = if current_active { 0 } else { 1 };

        // SAFELY isolate the mutable reference to the dormant buffer.
        let back_buffer = unsafe { 
            &mut (*self.core_buffer.buffers.get())[back_idx] 
        };

        fill_buffer(&mut back_buffer.data, &mut back_buffer.layout, &mut back_buffer.track_id);
        self.core_buffer.active_index.store(!current_active, Ordering::Release);
    }



    pub fn set_active_layout(&self, layout: TrackLayout) {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        unsafe { (*self.core_buffer.buffers.get())[active_idx].layout = layout };
    }

    pub fn get_active_half_track(&self) -> u8 {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        unsafe { (*self.core_buffer.buffers.get())[active_idx].track_id }
    }

    pub fn get_active_layout(&self) -> TrackLayout {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        unsafe { (*self.core_buffer.buffers.get())[active_idx].layout }
    }

    pub fn start_track_load(&self) -> &mut [u8] {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let back_idx = if current_active { 0 } else { 1 };
        unsafe { &mut (*self.core_buffer.buffers.get())[back_idx].data }
    }

    pub fn commit_loaded_track(&self, track_id: u8, layout: TrackLayout, length: usize) {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let back_idx = if current_active { 0 } else { 1 };
        let back_buffer = unsafe { &mut (*self.core_buffer.buffers.get())[back_idx] };
        
        back_buffer.layout = layout;
        back_buffer.length = length;
        back_buffer.track_id = track_id;
        
        self.core_buffer.active_index.store(!current_active, Ordering::Release);
    }

    pub fn start_track_modification(&self) -> &mut [u8] {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let active_idx = if current_active { 1 } else { 0 };
        let back_idx = if current_active { 0 } else { 1 };
        
        unsafe {
            let buffers_ptr = self.core_buffer.buffers.get();
            let active_buffer = &(*buffers_ptr)[active_idx];
            let back_buffer = &mut (*buffers_ptr)[back_idx];

            back_buffer.data.copy_from_slice(&active_buffer.data);
            back_buffer.layout = active_buffer.layout;
            back_buffer.track_id = active_buffer.track_id;
            back_buffer.length = active_buffer.length;
            
            &mut back_buffer.data
        }
    }

    pub fn commit_modified_track(&self) {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        self.core_buffer.active_index.store(!current_active, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn get_dormant_buffer_mut(&self) -> &mut TrackBuffer {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let back_idx = if current_active { 0 } else { 1 };
        unsafe { &mut (*self.core_buffer.buffers.get())[back_idx] }
    }

    #[allow(dead_code)]
    pub fn commit_dormant_buffer(&self) {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        self.core_buffer.active_index.store(!current_active, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn duplicate_active_to_inactive(&self) {
        let current_active = self.core_buffer.active_index.load(Ordering::Acquire);
        let active_idx = if current_active { 1 } else { 0 };
        let back_idx = if current_active { 0 } else { 1 };
        
        unsafe {
            let buffers_ptr = self.core_buffer.buffers.get();
            let active_buffer = &(*buffers_ptr)[active_idx];
            let back_buffer = &mut (*buffers_ptr)[back_idx];

            back_buffer.data.copy_from_slice(&active_buffer.data);
            back_buffer.layout = active_buffer.layout;
            back_buffer.track_id = active_buffer.track_id;
            back_buffer.length = active_buffer.length;
        }
    }

    pub fn set_write_protected(&self, wp: bool) {
        self.core_buffer.is_write_protected.store(wp, Ordering::Release);
    }

    pub fn set_force_disk_change(&self, force: bool) {
        self.core_buffer.force_disk_change.store(force, Ordering::Release);
    }

    pub fn get_current_byte_idx(&self) -> usize {
        self.core_buffer.current_byte_idx.load(Ordering::Acquire)
    }
}

// =====================================================================
// The EMULATOR handle
// =====================================================================
pub struct EmulatorHandle {
    core_buffer: &'static DualCoreTrackBuffer,
    pub track_byte_pos_ix: usize,
    pub track_byte_size: usize,
    last_requested_sector: u8,
    last_is_listening: bool,
}

impl EmulatorHandle {
    pub fn is_write_protected(&self) -> bool {
        self.core_buffer.is_write_protected.load(Ordering::Acquire)
    }

    pub fn pop_force_disk_change(&mut self) -> bool {
        if self.core_buffer.force_disk_change.load(Ordering::Acquire) {
            self.core_buffer.force_disk_change.store(false, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub fn get_current_byte_idx(&self) -> usize {
        self.core_buffer.current_byte_idx.load(Ordering::Acquire)
    }

    #[inline(always)]
    fn resolve_and_advance_index(&mut self, buffer: &TrackBuffer, expected_track_id: u8, requested_sector: u8, just_started_reading: bool) -> Option<usize> {
        if buffer.track_id != expected_track_id || buffer.length == 0 {
            self.last_requested_sector = 0xFF;
            return None;
        }

        let mut has_position = true;
        match buffer.layout {
            TrackLayout::Accurate => {
                if buffer.length != self.track_byte_size {
                    if self.track_byte_size > 0 {
                        // NOTE: RP2040 uses an 8-cycle hardware SIO divider for this operation.
                        // Executing this division once per track change is ~32ns, which is mathematically
                        // optimal compared to adding fixed-point math overhead to the per-byte hot path!
                        self.track_byte_pos_ix = (self.track_byte_pos_ix * buffer.length) / self.track_byte_size;
                    }
                    self.track_byte_size = buffer.length;
                }
            },
            TrackLayout::Warped(stride) => {
                self.track_byte_size = buffer.length;
                if requested_sector != self.last_requested_sector || just_started_reading {
                    self.track_byte_pos_ix = (requested_sector as usize) * stride;
                    self.last_requested_sector = requested_sector;
                }
            },
            TrackLayout::Void => {
                self.last_requested_sector = 0xFF;
                has_position = false;
            }
        }

        if self.track_byte_size > 0 && self.track_byte_pos_ix >= self.track_byte_size {
            self.track_byte_pos_ix = 0;
        }

        let current_byte_pos = self.track_byte_pos_ix;
        self.track_byte_pos_ix += 1;
        self.core_buffer.current_byte_idx.store(self.track_byte_pos_ix, Ordering::Relaxed);

        if has_position {
            Some(current_byte_pos % buffer.length)
        } else {
            None
        }
    }

    /// Safely reads a GCR byte. 
    /// Automatically handles the modulo math and returns 0x55 noise 
    /// if the head is in transit or unformatted.
    #[inline(always)]
    pub fn next_read_byte(&mut self, expected_track_id: u8, requested_sector: u8, is_listening: bool) -> u8 {
        // Detect transition
        let just_started_listening = is_listening && !self.last_is_listening;
        self.last_is_listening = is_listening;

        // 1. Lock-free active buffer resolution
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };

        // SAFETY: We only dereference the active index. 
        // Core 0 guarantees it only ever mutates the dormant index.
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };

        if let Some(ix) = self.resolve_and_advance_index(buffer, expected_track_id, requested_sector, just_started_listening) {
            buffer.data[ix]
        } else {
            VOID_BYTE
        }
    }

    #[inline(always)]
    pub fn peek_next_byte(&mut self) -> u8 {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };
        if buffer.length == 0 {
            return VOID_BYTE;
        }
        let mut ix = self.track_byte_pos_ix;
        if ix >= buffer.length {
            ix = 0;
        }
        buffer.data[ix]
    }

    /// Safely writes a GCR byte back to the disk buffer and flags it for background SD flush.
    #[inline(always)]
    pub fn next_write_byte(&mut self, expected_track_id: u8, requested_sector: u8, _data: u8) {
        let active_idx = if self.core_buffer.active_index.load(Ordering::Acquire) { 1 } else { 0 };
        
        let buffer = unsafe { &mut (*self.core_buffer.buffers.get())[active_idx] };

        // FIXME! Write support is not yet implemented. 
        // We only advance the index so the drive mechanics keep ticking correctly.
        let _ = self.resolve_and_advance_index(buffer, expected_track_id, requested_sector, false);
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

