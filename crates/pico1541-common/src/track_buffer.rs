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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TrackIdentifier(pub u16);

/// Converts a full CBM track (1-35) into its corresponding physical half-track index (0-68).
#[inline(always)]
pub const fn track_to_half_track(track: u8) -> u8 {
    (track.saturating_sub(1)) * 2
}

/// Converts a physical half-track index (0-68) into its corresponding full CBM track (1-35).
#[inline(always)]
pub const fn half_track_to_track(half_track: u8) -> u8 {
    half_track / 2 + 1
}

#[inline(always)]
pub const fn default_speed_zone(half_track: u8) -> u8 {
    let track = half_track_to_track(half_track);
    if track <= 17 {
        3
    } else if track <= 24 {
        2
    } else if track <= 30 {
        1
    } else {
        0
    }
}

/// Returns the standard exact track capacity in bytes for a given speed zone
#[inline]
pub const fn speed_zone_capacity(zone: u8) -> usize {
    match zone & 3 {
        3 => 7692,
        2 => 7142,
        1 => 6666,
        0 => 6250,
        _ => 6250,
    }
}

/// Returns the exact bitrate in bytes/sec for a given speed zone
#[inline]
pub const fn speed_zone_bitrate(zone: u8) -> u32 {
    match zone & 3 {
        3 => 38461, // 307.69 kbps / 8 (3.25 us/bit)
        2 => 35714, // 285.71 kbps / 8 (3.50 us/bit)
        1 => 33333, // 266.66 kbps / 8 (3.75 us/bit)
        0 => 31250, // 250.00 kbps / 8 (4.00 us/bit)
        _ => 31250,
    }
}

/// Returns the bit period in tenths of a nanosecond (0.1 ns) for a given speed zone
#[inline]
pub const fn speed_zone_bit_period_100ps(zone: u8) -> u32 {
    match zone & 3 {
        3 => 32500, // 3.25 us = 32500 * 0.1 ns
        2 => 35000,
        1 => 37500,
        0 => 40000,
        _ => 40000,
    }
}

impl TrackIdentifier {
    #[inline(always)]
    pub fn new(half_track: u8, speed_zone: u8) -> Self {
        Self(((speed_zone as u16) << 8) | (half_track as u16))
    }

    #[inline(always)]
    pub fn void() -> Self {
        Self(0xFF)
    }

    #[inline(always)]
    pub fn half_track(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    #[inline(always)]
    pub fn with_speed_zone(&self, speed_zone: u8) -> Self {
        Self(self.0 & 0xFF | (speed_zone as u16) << 8)
    }

    #[inline(always)]
    pub fn with_half_track(&self, half_track: u8) -> Self {
        Self(self.0 & 0xFF00 | (half_track as u16))
    }
}

/// Represents a single track buffer which holds the GCR-encoded data for a track,
/// along with metadata about the track such as its ID and layout.
#[repr(C, align(8192))]
#[repr(C, align(8192))]
pub struct DualCoreTrackData {
    pub buffers: core::cell::UnsafeCell<[[u8; TRACK_SIZE]; 2]>,
}

unsafe impl Sync for DualCoreTrackData {}
unsafe impl Send for DualCoreTrackData {}

impl DualCoreTrackData {
    pub const fn new() -> Self {
        Self {
            buffers: core::cell::UnsafeCell::new([[0; TRACK_SIZE], [0; TRACK_SIZE]]),
        }
    }
}

pub struct TrackBuffer {
    pub data: *mut [u8; TRACK_SIZE],
    pub identifier: TrackIdentifier,
    pub length: usize, // Valid bytes in the buffer
    pub layout: TrackLayout,
}

unsafe impl Sync for TrackBuffer {}
unsafe impl Send for TrackBuffer {}

impl TrackBuffer {
    pub const fn new(data: *mut [u8; TRACK_SIZE]) -> Self {
        Self {
            data,
            identifier: TrackIdentifier(0), // 0xFF represents invalid/unloaded
            length: 0,
            layout: TrackLayout::Void,
        }
    }

    pub const fn noise() -> &'static Self {
        #[repr(C, align(8192))]
        struct NoiseData([u8; 8192]);

        static mut NOISE_DATA: NoiseData = NoiseData({
            let mut arr = [0; 8192];
            arr[0] = 0x55;
            arr[1] = 0xAA;
            arr[2] = 0x33;
            arr[3] = 0xCC;
            arr[4] = 0x0F;
            arr[5] = 0xF0;
            arr[6] = 0x55;
            arr[7] = 0xAA;
            arr[8] = 0x33;
            arr[9] = 0xCC;
            arr[10] = 0x0F;
            arr[11] = 0xF0;
            arr[12] = 0x55;
            arr[13] = 0xAA;
            arr[14] = 0x33;
            arr[15] = 0xCC;
            arr
        });

        static NOISE_BUFFER: TrackBuffer = TrackBuffer {
            data: unsafe { core::ptr::addr_of_mut!(NOISE_DATA.0) },
            identifier: TrackIdentifier(0xFF),
            length: 16,
            layout: TrackLayout::Void,
        };
        &NOISE_BUFFER
    }
}

/// Dual core track buffer that can be split into two handles for the Emulator and Manager
pub struct DualCoreTrackBuffer {
    // Now holding our unified struct
    buffers: UnsafeCell<[TrackBuffer; 2]>,
    generation: core::sync::atomic::AtomicU8,
    is_write_protected: AtomicBool,
    force_disk_change: AtomicBool,
    current_byte_idx: core::sync::atomic::AtomicUsize,
    current_head_half_track: core::sync::atomic::AtomicU8,
}

unsafe impl Sync for DualCoreTrackBuffer {}
unsafe impl Send for DualCoreTrackBuffer {}

impl DualCoreTrackBuffer {
    pub const fn new(data: *mut [[u8; TRACK_SIZE]; 2]) -> Self {
        let ptr0 = data.cast::<[u8; TRACK_SIZE]>();
        let ptr1 = unsafe { ptr0.add(1) };
        Self {
            buffers: UnsafeCell::new([TrackBuffer::new(ptr0), TrackBuffer::new(ptr1)]),
            generation: core::sync::atomic::AtomicU8::new(0),
            is_write_protected: AtomicBool::new(false),
            force_disk_change: AtomicBool::new(false),
            current_byte_idx: core::sync::atomic::AtomicUsize::new(0),
            current_head_half_track: core::sync::atomic::AtomicU8::new(34),
        }
    }

    pub fn reset(&self, data: *mut [[u8; crate::track_buffer::TRACK_SIZE]; 2]) {
        self.generation
            .store(0, core::sync::atomic::Ordering::Relaxed);
        self.is_write_protected
            .store(false, core::sync::atomic::Ordering::Relaxed);
        self.force_disk_change
            .store(false, core::sync::atomic::Ordering::Relaxed);
        self.current_byte_idx
            .store(0, core::sync::atomic::Ordering::Relaxed);
        self.current_head_half_track
            .store(34, core::sync::atomic::Ordering::Relaxed);
        unsafe {
            let bufs = &mut *self.buffers.get();
            let ptr0 = data.cast::<[u8; crate::track_buffer::TRACK_SIZE]>();
            let ptr1 = ptr0.add(1);
            bufs[0].data = ptr0;
            bufs[1].data = ptr1;
            for buf in bufs.iter_mut() {
                buf.identifier = TrackIdentifier(0xFF);
                buf.length = 0;
                buf.layout = TrackLayout::Void;
            }
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
    pub core_buffer: &'static DualCoreTrackBuffer,
}

impl ManagerHandle {
    pub fn get_head_half_track(&self) -> u8 {
        self.core_buffer
            .current_head_half_track
            .load(Ordering::Relaxed)
    }

    pub fn load_next_track<F>(&self, fill_buffer: F)
    where
        F: FnOnce(&mut [u8; TRACK_SIZE], &mut TrackLayout, &mut TrackIdentifier),
    {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let back_idx = ((generation % 2) ^ 1) as usize;

        // SAFELY isolate the mutable reference to the dormant buffer.
        let back_buffer = unsafe { &mut (*self.core_buffer.buffers.get())[back_idx] };

        fill_buffer(
            unsafe { &mut *back_buffer.data },
            &mut back_buffer.layout,
            &mut back_buffer.identifier,
        );
        self.core_buffer
            .generation
            .store(generation.wrapping_add(1), Ordering::Release);
    }

    pub fn set_active_layout(&self, layout: TrackLayout) {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        unsafe { (*self.core_buffer.buffers.get())[active_idx].layout = layout };
    }

    pub fn set_active_track_identifier(&self, identifier: TrackIdentifier) {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        unsafe { (*self.core_buffer.buffers.get())[active_idx].identifier = identifier };
    }

    pub fn get_active_track_identifier(&self) -> TrackIdentifier {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        unsafe { (*self.core_buffer.buffers.get())[active_idx].identifier }
    }

    pub fn get_active_layout(&self) -> TrackLayout {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        unsafe { (*self.core_buffer.buffers.get())[active_idx].layout }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn start_track_load(&self) -> &mut [u8] {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let back_idx = ((generation % 2) ^ 1) as usize;
        unsafe { &mut *(*self.core_buffer.buffers.get())[back_idx].data }
    }

    pub fn commit_loaded_track(
        &self,
        identifier: TrackIdentifier,
        layout: TrackLayout,
        length: usize,
    ) {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let back_idx = ((generation % 2) ^ 1) as usize;
        let back_buffer = unsafe { &mut (*self.core_buffer.buffers.get())[back_idx] };

        back_buffer.layout = layout;

        // Pad the remainder of the 8192-byte buffer with standard GCR gap byte
        if length < 8192 {
            unsafe { (&mut *back_buffer.data)[length..8192].fill(0x55) };
        }

        let mut final_length = length;
        if final_length == 0 {
            // An empty track (e.g. half-tracks) should still produce random noise or gap bytes.
            final_length = 8000;
        }

        back_buffer.identifier = identifier;
        back_buffer.length = final_length;

        // Atomically switch active buffer
        self.core_buffer
            .generation
            .store(generation.wrapping_add(1), Ordering::Release);
    }

    #[allow(clippy::mut_from_ref)]
    pub fn start_track_modification(&self) -> &mut [u8; TRACK_SIZE] {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let active_idx = (generation % 2) as usize;
        let back_idx = ((generation % 2) ^ 1) as usize;

        unsafe {
            let buffers_ptr = self.core_buffer.buffers.get();
            let active_buffer = &(*buffers_ptr)[active_idx];
            let back_buffer = &mut (*buffers_ptr)[back_idx];

            (*back_buffer.data).copy_from_slice(&*active_buffer.data);
            back_buffer.layout = active_buffer.layout;
            back_buffer.identifier = active_buffer.identifier;
            back_buffer.length = active_buffer.length;

            &mut *back_buffer.data
        }
    }

    pub fn commit_modified_track(&self) {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        self.core_buffer
            .generation
            .store(generation.wrapping_add(1), Ordering::Release);
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get_dormant_buffer_mut(&self) -> &mut TrackBuffer {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let back_idx = ((generation % 2) ^ 1) as usize;
        unsafe { &mut (*self.core_buffer.buffers.get())[back_idx] }
    }

    pub fn commit_dormant_buffer(&self) {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        self.core_buffer
            .generation
            .store(generation.wrapping_add(1), Ordering::Release);
    }

    pub fn duplicate_active_to_inactive(&self) {
        let generation = self.core_buffer.generation.load(Ordering::Acquire);
        let active_idx = (generation % 2) as usize;
        let back_idx = ((generation % 2) ^ 1) as usize;

        unsafe {
            let buffers_ptr = self.core_buffer.buffers.get();
            let active_buffer = &(*buffers_ptr)[active_idx];
            let back_buffer = &mut (*buffers_ptr)[back_idx];

            (*back_buffer.data).copy_from_slice(&*active_buffer.data);
            back_buffer.layout = active_buffer.layout;
            back_buffer.identifier = active_buffer.identifier;
            back_buffer.length = active_buffer.length;
        }
    }

    pub fn set_write_protected(&self, wp: bool) {
        self.core_buffer
            .is_write_protected
            .store(wp, Ordering::Release);
    }

    pub fn set_force_disk_change(&self, force: bool) {
        self.core_buffer
            .force_disk_change
            .store(force, Ordering::Release);
    }

    pub fn force_disk_change(&self) {
        self.set_force_disk_change(true);
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
    pub fn update_head_half_track(&self, ht: u8) {
        self.core_buffer
            .current_head_half_track
            .store(ht, Ordering::Relaxed);
    }

    pub fn get_head_half_track(&self) -> u8 {
        self.core_buffer
            .current_head_half_track
            .load(Ordering::Relaxed)
    }

    pub fn is_write_protected(&self) -> bool {
        self.core_buffer.is_write_protected.load(Ordering::Acquire)
    }

    pub fn pop_force_disk_change(&mut self) -> bool {
        if self.core_buffer.force_disk_change.load(Ordering::Acquire) {
            self.core_buffer
                .force_disk_change
                .store(false, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub fn get_current_byte_idx(&self) -> usize {
        self.core_buffer.current_byte_idx.load(Ordering::Acquire)
    }

    #[inline(always)]
    pub fn get_generation(&self) -> u8 {
        self.core_buffer.generation.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn get_active_track_identifier(&self) -> TrackIdentifier {
        let active_idx = (self.get_generation() % 2) as usize;
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };
        buffer.identifier
    }

    pub fn get_active_track_length(&self) -> usize {
        let active_idx = (self.get_generation() % 2) as usize;
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };
        buffer.length
    }

    pub fn get_active_buffer_slice(&self) -> &[u8] {
        let active_idx = (self.get_generation() % 2) as usize;
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };
        unsafe { &(&*buffer.data)[..buffer.length] }
    }

    pub fn get_active_track_buffer(&self) -> &'static TrackBuffer {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        unsafe { &(*self.core_buffer.buffers.get())[active_idx] }
    }

    pub fn seek_to_byte(&mut self, byte_index: u32) {
        self.track_byte_pos_ix = byte_index as usize;
        self.core_buffer
            .current_byte_idx
            .store(self.track_byte_pos_ix, Ordering::Relaxed);
    }

    #[inline(always)]
    fn resolve_and_advance_index(
        &mut self,
        buffer: &TrackBuffer,
        expected_identifier: TrackIdentifier,
        requested_sector: u8,
        just_started_reading: bool,
    ) -> Option<usize> {
        if buffer.identifier != expected_identifier || buffer.length == 0 {
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
                        self.track_byte_pos_ix =
                            (self.track_byte_pos_ix * buffer.length) / self.track_byte_size;
                    }
                    self.track_byte_size = buffer.length;
                }
            }
            TrackLayout::Warped(stride) => {
                self.track_byte_size = buffer.length;
                if requested_sector != self.last_requested_sector || just_started_reading {
                    self.track_byte_pos_ix = (requested_sector as usize) * stride;
                    self.last_requested_sector = requested_sector;
                }
            }
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
        self.core_buffer
            .current_byte_idx
            .store(self.track_byte_pos_ix, Ordering::Relaxed);

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
    pub fn next_read_byte(
        &mut self,
        expected_identifier: TrackIdentifier,
        requested_sector: u8,
        is_listening: bool,
    ) -> u8 {
        // Detect transition
        let just_started_listening = is_listening && !self.last_is_listening;
        self.last_is_listening = is_listening;

        // 1. Lock-free active buffer resolution
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;

        // SAFETY: We only dereference the active index.
        // Core 0 guarantees it only ever mutates the dormant index.
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };

        if let Some(ix) = self.resolve_and_advance_index(
            buffer,
            expected_identifier,
            requested_sector,
            just_started_listening,
        ) {
            unsafe { (*buffer.data)[ix] }
        } else {
            VOID_BYTE
        }
    }

    #[inline(always)]
    pub fn peek_next_byte(&mut self) -> u8 {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };
        if buffer.length == 0 {
            return VOID_BYTE;
        }
        let mut ix = self.track_byte_pos_ix;
        if ix >= buffer.length {
            ix = 0;
        }
        unsafe { (*buffer.data)[ix] }
    }

    /// Safely writes a GCR byte back to the disk buffer and flags it for background SD flush.
    #[inline(always)]
    pub fn next_write_byte(
        &mut self,
        expected_identifier: TrackIdentifier,
        requested_sector: u8,
        _data: u8,
    ) {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;

        let buffer = unsafe { &mut (*self.core_buffer.buffers.get())[active_idx] };

        // FIXME! Write support is not yet implemented.
        // We only advance the index so the drive mechanics keep ticking correctly.
        let _ =
            self.resolve_and_advance_index(buffer, expected_identifier, requested_sector, false);
    }

    /// Fetches the track layout to calculate bit-rates (Speed Zones).
    /// Returns None if the track is in transit.
    #[inline(always)]
    pub fn get_layout(&self, expected_identifier: TrackIdentifier) -> Option<TrackLayout> {
        let active_idx = (self.core_buffer.generation.load(Ordering::Acquire) % 2) as usize;
        let buffer = unsafe { &(*self.core_buffer.buffers.get())[active_idx] };

        if buffer.identifier == expected_identifier {
            Some(buffer.layout)
        } else {
            None
        }
    }
}
