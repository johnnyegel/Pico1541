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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct WeakBitRange {
    pub offset: u16,
    pub length: u16,
}

pub const MAX_WEAK_RANGES: usize = 8;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Lfsr16(pub u16);

impl Lfsr16 {
    pub const fn new() -> Self {
        Self(0xACE1)
    }

    pub const fn with_seed(seed: u16) -> Self {
        Self(if seed == 0 { 0xACE1 } else { seed })
    }

    #[inline(always)]
    pub fn next_u8(&mut self) -> u8 {
        let bit = self.0 & 1;
        self.0 = (self.0 >> 1) ^ (if bit != 0 { 0xB400 } else { 0 });
        (self.0 & 0xFF) as u8
    }

    #[inline(always)]
    pub fn next_weak_byte(&mut self) -> u8 {
        // Mask with 0x7E (0b0111_1110) to clear bit 7 and bit 0.
        // Standard GCR bytes before/after the weak range can end/start with up to three 1-bits.
        // Clearing both bit 7 and bit 0 ensures the byte has at most 6 consecutive 1-bits
        // and cannot produce 10 consecutive 1s (false SYNC) across byte boundaries.
        let b = self.next_u8() & 0x7E;
        if b == 0 { 0x54 } else { b } // ensure not 0x00 either (0x54 has bit 7 and bit 0 as 0)
    }
}

impl Default for Lfsr16 {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Maps a virtual directory slot index to its CBM track and sector.
/// Tracks run from 17 down to 11 (clamped so underflow is impossible), each with 21 sectors.
#[inline(always)]
pub fn slot_to_track_sector(slot: usize) -> (u8, u8) {
    let track = (17 - (slot / 21).min(6)) as u8;
    let sector = (slot % 21) as u8;
    (track, sector)
}

/// Maps a CBM track and sector to its virtual directory slot index.
/// Returns Some(slot) if track is in 11..=17 and sector < 21, otherwise None.
#[inline(always)]
pub fn track_sector_to_slot(track: u8, sector: u8) -> Option<usize> {
    if track >= 11 && track <= 17 && sector < 21 {
        Some((17 - track) as usize * 21 + sector as usize)
    } else {
        None
    }
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
    pub weak_ranges: [Option<WeakBitRange>; MAX_WEAK_RANGES],
    pub weak_range_count: u8,
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
            weak_ranges: [None; MAX_WEAK_RANGES],
            weak_range_count: 0,
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
            weak_ranges: [None; MAX_WEAK_RANGES],
            weak_range_count: 0,
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
            current_byte_idx: core::sync::atomic::AtomicUsize::new(0),
            current_head_half_track: core::sync::atomic::AtomicU8::new(34),
        }
    }

    pub fn reset(&self, data: *mut [[u8; crate::track_buffer::TRACK_SIZE]; 2]) {
        self.generation
            .store(0, core::sync::atomic::Ordering::Relaxed);
        self.is_write_protected
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
                buf.weak_ranges.fill(None);
                buf.weak_range_count = 0;
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

        back_buffer.weak_ranges.fill(None);
        back_buffer.weak_range_count = 0;

        if layout == TrackLayout::Accurate {
            let mut lfsr = Lfsr16::new();
            let scan_len = length.min(TRACK_SIZE);
            let data_slice = unsafe { &mut (&mut *back_buffer.data)[..scan_len] };
            let mut i = 0;
            while i < data_slice.len() {
                if data_slice[i] == 0x00 {
                    let start = i;
                    while i < data_slice.len() && data_slice[i] == 0x00 {
                        data_slice[i] = lfsr.next_weak_byte();
                        i += 1;
                    }
                    let run_len = i - start;
                    if (back_buffer.weak_range_count as usize) < MAX_WEAK_RANGES {
                        back_buffer.weak_ranges[back_buffer.weak_range_count as usize] =
                            Some(WeakBitRange {
                                offset: start as u16,
                                length: run_len as u16,
                            });
                        back_buffer.weak_range_count += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }

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
            back_buffer.weak_ranges = active_buffer.weak_ranges;
            back_buffer.weak_range_count = active_buffer.weak_range_count;

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
            back_buffer.weak_ranges = active_buffer.weak_ranges;
            back_buffer.weak_range_count = active_buffer.weak_range_count;
        }
    }

    pub fn set_write_protected(&self, wp: bool) {
        self.core_buffer
            .is_write_protected
            .store(wp, Ordering::Release);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_to_track_sector_and_inverse() {
        assert_eq!(slot_to_track_sector(0), (17, 0));
        assert_eq!(slot_to_track_sector(20), (17, 20));
        assert_eq!(slot_to_track_sector(21), (16, 0));
        assert_eq!(slot_to_track_sector(146), (11, 20));

        // Overflow clamp test
        assert_eq!(slot_to_track_sector(200), (11, 11));

        for slot in 0..(7 * 21) {
            let (track, sector) = slot_to_track_sector(slot);
            assert!(track >= 11 && track <= 17);
            assert!(sector < 21);
            assert_eq!(track_sector_to_slot(track, sector), Some(slot));
        }

        // Invalid track/sector inputs
        assert_eq!(track_sector_to_slot(18, 0), None);
        assert_eq!(track_sector_to_slot(10, 0), None);
        assert_eq!(track_sector_to_slot(17, 21), None);
    }

    #[test]
    fn test_lfsr16_noise_properties() {
        let mut lfsr = Lfsr16::new();
        assert_eq!(lfsr.0, 0xACE1);

        let custom = Lfsr16::with_seed(0x1234);
        assert_eq!(custom.0, 0x1234);

        let zero_seed = Lfsr16::with_seed(0);
        assert_eq!(zero_seed.0, 0xACE1);

        let default_lfsr = Lfsr16::default();
        assert_eq!(default_lfsr.0, 0xACE1);

        for _ in 0..10_000 {
            let byte = lfsr.next_weak_byte();
            assert_ne!(byte, 0x00, "Weak byte must not be 0x00");
            assert_ne!(byte, 0xFF, "Weak byte must not be 0xFF to prevent false SYNC");
            assert_eq!(
                byte & 0x81,
                0,
                "Weak byte bit 7 and bit 0 must be 0 (masked with 0x7E) to avoid boundary false SYNC"
            );
        }
    }

    #[test]
    fn test_commit_loaded_track_weak_ranges_and_replacement() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        // Populate track buffer with non-zero data and two runs of 0x00
        let buf = manager.start_track_load();
        buf.fill(0x55);
        // 5 bytes of 0x00 at offset 100
        buf[100..105].fill(0x00);
        // 10 bytes of 0x00 at offset 500
        buf[500..510].fill(0x00);

        manager.commit_loaded_track(TrackIdentifier(1), TrackLayout::Accurate, 600);

        let active_buf = emulator.get_active_track_buffer();
        assert_eq!(active_buf.weak_range_count, 2);
        assert_eq!(
            active_buf.weak_ranges[0],
            Some(WeakBitRange {
                offset: 100,
                length: 5
            })
        );
        assert_eq!(
            active_buf.weak_ranges[1],
            Some(WeakBitRange {
                offset: 500,
                length: 10
            })
        );
        for i in 2..MAX_WEAK_RANGES {
            assert_eq!(active_buf.weak_ranges[i], None);
        }

        // Verify the 0x00 bytes were replaced with pseudo-random bytes != 0x00 and != 0xFF, with bits 7 and 0 cleared
        let slice = emulator.get_active_buffer_slice();
        for &b in &slice[100..105] {
            assert_ne!(b, 0x00);
            assert_ne!(b, 0xFF);
            assert_eq!(b & 0x81, 0);
        }
        for &b in &slice[500..510] {
            assert_ne!(b, 0x00);
            assert_ne!(b, 0xFF);
            assert_eq!(b & 0x81, 0);
        }
        // Verify surrounding bytes are still 0x55
        assert_eq!(slice[99], 0x55);
        assert_eq!(slice[105], 0x55);
        assert_eq!(slice[499], 0x55);
        assert_eq!(slice[510], 0x55);
    }

    #[test]
    fn test_commit_loaded_track_clean_gcr() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        let buf = manager.start_track_load();
        buf.fill(0x55);

        manager.commit_loaded_track(TrackIdentifier(2), TrackLayout::Accurate, 600);

        let active_buf = emulator.get_active_track_buffer();
        assert_eq!(active_buf.weak_range_count, 0);
        for range in active_buf.weak_ranges.iter() {
            assert_eq!(*range, None);
        }
        assert_eq!(&emulator.get_active_buffer_slice()[..600], &[0x55; 600]);
    }

    #[test]
    fn test_commit_loaded_track_non_accurate_layout() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        let buf = manager.start_track_load();
        buf.fill(0x00); // All 0x00, but layout is Warped

        manager.commit_loaded_track(TrackIdentifier(3), TrackLayout::Warped(300), 600);

        let active_buf = emulator.get_active_track_buffer();
        assert_eq!(active_buf.weak_range_count, 0);
        for range in active_buf.weak_ranges.iter() {
            assert_eq!(*range, None);
        }
    }

    #[test]
    fn test_duplicate_active_to_inactive_preserves_weak_ranges() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        let buf = manager.start_track_load();
        buf.fill(0x55);
        buf[200..208].fill(0x00);
        manager.commit_loaded_track(TrackIdentifier(4), TrackLayout::Accurate, 500);

        assert_eq!(emulator.get_active_track_buffer().weak_range_count, 1);
        assert_eq!(
            emulator.get_active_track_buffer().weak_ranges[0],
            Some(WeakBitRange {
                offset: 200,
                length: 8
            })
        );

        // Duplicate active to dormant/inactive
        manager.duplicate_active_to_inactive();
        let dormant = manager.get_dormant_buffer_mut();
        assert_eq!(dormant.weak_range_count, 1);
        assert_eq!(
            dormant.weak_ranges[0],
            Some(WeakBitRange {
                offset: 200,
                length: 8
            })
        );
    }

    #[test]
    fn test_max_weak_ranges_overflow() {
        let data = Box::into_raw(Box::new([[0u8; 8192]; 2]));
        let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(data)));
        let (manager, emulator) = dual_buffer.split();

        let buf = manager.start_track_load();
        buf.fill(0x55);
        // Create 10 runs of 0x00 (more than MAX_WEAK_RANGES = 8)
        for r in 0..10 {
            let offset = r * 20;
            buf[offset..offset + 5].fill(0x00);
        }

        manager.commit_loaded_track(TrackIdentifier(5), TrackLayout::Accurate, 300);

        let active_buf = emulator.get_active_track_buffer();
        assert_eq!(active_buf.weak_range_count, MAX_WEAK_RANGES as u8);
        for r in 0..MAX_WEAK_RANGES {
            assert_eq!(
                active_buf.weak_ranges[r],
                Some(WeakBitRange {
                    offset: (r * 20) as u16,
                    length: 5
                })
            );
        }

        // All 10 runs should have their 0x00 bytes replaced
        let slice = emulator.get_active_buffer_slice();
        for r in 0..10 {
            let offset = r * 20;
            for &b in &slice[offset..offset + 5] {
                assert_ne!(b, 0x00);
                assert_ne!(b, 0xFF);
            }
        }
    }
}
