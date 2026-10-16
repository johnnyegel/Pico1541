//! G64 container builder and binary serializer for Commodore 1541 disk images.
//!
//! G64 is a raw GCR disk image format used by Commodore 64 emulators (e.g. VICE)
//! and hardware emulators (such as Pico1541). It supports up to 84 half-tracks,
//! variable track lengths, and speed zone specifications.
//!
//! # Binary Layout
//! - Header (12 bytes, offset `0x0000..0x000C`):
//!   - `0x00..0x08`: Magic signature `b"GCR-1541"` (8 bytes)
//!   - `0x08`: Version `0x00` (1 byte)
//!   - `0x09`: Total half-tracks `84` / `0x54` (1 byte)
//!   - `0x0A..0x0C`: Max track size in bytes (`u16` little-endian, default 7928)
//! - Track Offset Table (336 bytes, offset `0x000C..0x015C`):
//!   - 84 × `u32` little-endian file offsets to each track data block (or 0 if unformatted).
//! - Speed Zone Table (336 bytes, offset `0x015C..0x02AC`):
//!   - 84 × `u32` little-endian speed zone indicators (`0..=3`).
//! - Track Payloads (sequential blocks starting at offset `0x02AC`):
//!   - 2 bytes: track length `L` (`u16` little-endian)
//!   - `L` bytes: raw GCR track data

/// Magic identifier for G64 disk images.
pub const MAGIC: &[u8; 8] = b"GCR-1541";

/// G64 format version (0x00).
pub const VERSION: u8 = 0x00;

/// Maximum number of half-tracks supported by G64 (84 half-tracks = tracks 1.0 to 42.5).
pub const MAX_TRACKS: usize = 84;

/// Default maximum track length in bytes (7,928 bytes).
pub const DEFAULT_MAX_TRACK_LEN: u16 = 7928;

/// Size of the G64 header in bytes.
pub const HEADER_SIZE: usize = 12;

/// File offset where the track offset table starts (12 / 0x000C).
pub const OFFSET_TABLE_OFFSET: usize = 0x000C;

/// Size of the track offset table in bytes (84 * 4 = 336 bytes).
pub const OFFSET_TABLE_SIZE: usize = MAX_TRACKS * 4;

/// File offset where the speed zone table starts (348 / 0x015C).
pub const SPEED_TABLE_OFFSET: usize = 0x015C;

/// Size of the speed zone table in bytes (84 * 4 = 336 bytes).
pub const SPEED_TABLE_SIZE: usize = MAX_TRACKS * 4;

/// File offset where track data payloads start (684 bytes / 0x02AC).
pub const TRACK_DATA_OFFSET: usize = HEADER_SIZE + OFFSET_TABLE_SIZE + SPEED_TABLE_SIZE;

/// Returns the standard Commodore 1541 speed zone (0..=3) for a given half-track index (0..=83).
///
/// Tracks 1..=17  (half-tracks 0..=33)  -> Speed Zone 3 (300 rpm / 300 kbit/s)
/// Tracks 18..=24 (half-tracks 34..=47) -> Speed Zone 2 (300 rpm / 275 kbit/s)
/// Tracks 25..=30 (half-tracks 48..=59) -> Speed Zone 1 (300 rpm / 250 kbit/s)
/// Tracks 31..=42 (half-tracks 60..=83) -> Speed Zone 0 (300 rpm / 225 kbit/s)
pub fn standard_speed_zone(half_track: u8) -> u8 {
    let track = (half_track / 2) + 1;
    match track {
        1..=17 => 3,
        18..=24 => 2,
        25..=30 => 1,
        _ => 0,
    }
}

/// In-memory builder for constructing and serializing G64 disk images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct G64Builder {
    tracks: [Option<(u8, Vec<u8>)>; MAX_TRACKS],
}

impl G64Builder {
    /// Creates a new empty G64 image builder with all 84 half-tracks unformatted.
    pub fn new() -> Self {
        Self {
            tracks: std::array::from_fn(|_| None),
        }
    }

    /// Sets or updates the track data for a given half-track index (0..=83).
    ///
    /// If `half_track >= 84`, this call is safely ignored without panicking.
    /// The speed zone is masked with `0x03` to ensure a valid range of 0..=3.
    pub fn set_track(&mut self, half_track: u8, speed_zone: u8, data: &[u8]) {
        let index = half_track as usize;
        if index < MAX_TRACKS {
            self.tracks[index] = Some((speed_zone & 0x03, data.to_vec()));
        }
    }

    /// Removes a track, marking it as unformatted / absent.
    pub fn clear_track(&mut self, half_track: u8) {
        let index = half_track as usize;
        if index < MAX_TRACKS {
            self.tracks[index] = None;
        }
    }

    /// Returns a reference to the track entry if present: `(speed_zone, track_data)`.
    pub fn track(&self, half_track: u8) -> Option<(u8, &[u8])> {
        let index = half_track as usize;
        if index < MAX_TRACKS {
            self.tracks[index].as_ref().map(|(sz, d)| (*sz, d.as_slice()))
        } else {
            None
        }
    }

    /// Returns the number of formatted / present tracks.
    pub fn track_count(&self) -> usize {
        self.tracks.iter().filter(|t| t.is_some()).count()
    }

    /// Serializes the G64 disk image to raw binary format.
    ///
    /// Serializes:
    /// - 12-byte G64 header
    /// - 336-byte track offset table
    /// - 336-byte speed zone table
    /// - Sequential track data payloads (2-byte length prefix + GCR data)
    pub fn serialize(&self) -> Vec<u8> {
        let mut offsets = [0u32; MAX_TRACKS];
        let mut speed_zones = [0u32; MAX_TRACKS];
        let mut current_offset = TRACK_DATA_OFFSET;

        for (ht, track_opt) in self.tracks.iter().enumerate() {
            if let Some((speed_zone, data)) = track_opt {
                offsets[ht] = current_offset as u32;
                speed_zones[ht] = *speed_zone as u32;
                current_offset += 2 + data.len();
            } else {
                offsets[ht] = 0;
                speed_zones[ht] = standard_speed_zone(ht as u8) as u32;
            }
        }

        // Calculate max_track_len: maximum of all present track lengths, or DEFAULT_MAX_TRACK_LEN if larger or empty.
        let max_len = self
            .tracks
            .iter()
            .flatten()
            .map(|(_, data)| data.len())
            .max()
            .unwrap_or(0);
        let max_track_len = (max_len as u16).max(DEFAULT_MAX_TRACK_LEN);

        let total_payload_len: usize = self
            .tracks
            .iter()
            .flatten()
            .map(|(_, data)| 2 + data.len())
            .sum();
        let total_size = TRACK_DATA_OFFSET + total_payload_len;
        let mut buffer = Vec::with_capacity(total_size);

        // 1. Header (12 bytes, offset 0x00..0x0C)
        buffer.extend_from_slice(MAGIC);
        buffer.push(VERSION);
        buffer.push(MAX_TRACKS as u8);
        buffer.extend_from_slice(&max_track_len.to_le_bytes());

        // 2. Track Offset Table (336 bytes, offset 0x0C..0x015C)
        for offset in &offsets {
            buffer.extend_from_slice(&offset.to_le_bytes());
        }

        // 3. Speed Zone Table (336 bytes, offset 0x015C..0x02AC)
        for zone in &speed_zones {
            buffer.extend_from_slice(&zone.to_le_bytes());
        }

        // 4. Track Data Payloads (starting at offset 0x02AC)
        for (_, data) in self.tracks.iter().flatten() {
            let len = data.len() as u16;
            buffer.extend_from_slice(&len.to_le_bytes());
            buffer.extend_from_slice(data);
        }

        buffer
    }
}

impl Default for G64Builder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_builder_serialization() {
        let builder = G64Builder::new();
        assert_eq!(builder.track_count(), 0);

        let bytes = builder.serialize();
        assert_eq!(bytes.len(), TRACK_DATA_OFFSET); // 684 bytes

        // Header checks
        assert_eq!(&bytes[0..8], MAGIC);
        assert_eq!(bytes[8], VERSION);
        assert_eq!(bytes[9], MAX_TRACKS as u8);
        assert_eq!(
            u16::from_le_bytes([bytes[10], bytes[11]]),
            DEFAULT_MAX_TRACK_LEN
        );

        // Offset table: all 84 entries should be 0
        for i in 0..MAX_TRACKS {
            let offset_pos = OFFSET_TABLE_OFFSET + i * 4;
            let offset = u32::from_le_bytes(bytes[offset_pos..offset_pos + 4].try_into().unwrap());
            assert_eq!(offset, 0, "Expected offset 0 at track {}", i);
        }

        // Speed table: all 84 entries should have standard speed zones
        for i in 0..MAX_TRACKS {
            let speed_pos = SPEED_TABLE_OFFSET + i * 4;
            let speed = u32::from_le_bytes(bytes[speed_pos..speed_pos + 4].try_into().unwrap());
            assert_eq!(
                speed,
                standard_speed_zone(i as u8) as u32,
                "Expected standard speed zone at track {}",
                i
            );
        }
    }

    #[test]
    fn test_single_track_serialization() {
        let mut builder = G64Builder::new();
        let track_data = vec![0x55; 100];
        builder.set_track(0, 3, &track_data);

        assert_eq!(builder.track_count(), 1);
        let track = builder.track(0).expect("Track 0 should be present");
        assert_eq!(track.0, 3);
        assert_eq!(track.1, track_data.as_slice());

        let bytes = builder.serialize();
        assert_eq!(bytes.len(), TRACK_DATA_OFFSET + 2 + 100);

        // Track 0 offset in table should be TRACK_DATA_OFFSET (684 / 0x02AC)
        let t0_offset = u32::from_le_bytes(
            bytes[OFFSET_TABLE_OFFSET..OFFSET_TABLE_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(t0_offset, TRACK_DATA_OFFSET as u32);

        // Track 0 speed zone should be 3
        let t0_speed = u32::from_le_bytes(
            bytes[SPEED_TABLE_OFFSET..SPEED_TABLE_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(t0_speed, 3);

        // Other tracks should have offset 0
        for i in 1..MAX_TRACKS {
            let offset_pos = OFFSET_TABLE_OFFSET + i * 4;
            let offset = u32::from_le_bytes(bytes[offset_pos..offset_pos + 4].try_into().unwrap());
            assert_eq!(offset, 0);
        }

        // Payload at TRACK_DATA_OFFSET
        let payload_len = u16::from_le_bytes([bytes[684], bytes[685]]);
        assert_eq!(payload_len, 100);
        assert_eq!(&bytes[686..686 + 100], &track_data[..]);
    }

    #[test]
    fn test_multi_track_serialization() {
        let mut builder = G64Builder::new();

        // Half-track 0 (Track 1.0): speed zone 3, 50 bytes
        let t0_data = vec![0x11; 50];
        builder.set_track(0, 3, &t0_data);

        // Half-track 34 (Track 18.0): speed zone 2, 80 bytes
        let t34_data = vec![0x22; 80];
        builder.set_track(34, 2, &t34_data);

        assert_eq!(builder.track_count(), 2);
        let bytes = builder.serialize();

        let expected_size = TRACK_DATA_OFFSET + (2 + 50) + (2 + 80);
        assert_eq!(bytes.len(), expected_size);

        // Verify Header
        assert_eq!(&bytes[0..8], MAGIC);
        assert_eq!(bytes[8], 0);
        assert_eq!(bytes[9], 84);
        assert_eq!(
            u16::from_le_bytes([bytes[10], bytes[11]]),
            DEFAULT_MAX_TRACK_LEN
        );

        // Offset table
        let off0 = u32::from_le_bytes(
            bytes[OFFSET_TABLE_OFFSET..OFFSET_TABLE_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(off0, 684);

        let off34_pos = OFFSET_TABLE_OFFSET + 34 * 4;
        let off34 = u32::from_le_bytes(bytes[off34_pos..off34_pos + 4].try_into().unwrap());
        let expected_off34 = 684 + 2 + 50;
        assert_eq!(off34, expected_off34 as u32);

        // Speed table
        let sz0 = u32::from_le_bytes(
            bytes[SPEED_TABLE_OFFSET..SPEED_TABLE_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(sz0, 3);

        let sz34_pos = SPEED_TABLE_OFFSET + 34 * 4;
        let sz34 = u32::from_le_bytes(bytes[sz34_pos..sz34_pos + 4].try_into().unwrap());
        assert_eq!(sz34, 2);

        // Verify unformatted tracks
        for i in 1..34 {
            let off = u32::from_le_bytes(
                bytes[OFFSET_TABLE_OFFSET + i * 4..OFFSET_TABLE_OFFSET + (i + 1) * 4]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(off, 0);
        }

        // Verify payloads
        // Track 0
        let len0 = u16::from_le_bytes([bytes[off0 as usize], bytes[off0 as usize + 1]]);
        assert_eq!(len0, 50);
        assert_eq!(
            &bytes[off0 as usize + 2..off0 as usize + 2 + 50],
            t0_data.as_slice()
        );

        // Track 34
        let len34 = u16::from_le_bytes([bytes[off34 as usize], bytes[off34 as usize + 1]]);
        assert_eq!(len34, 80);
        assert_eq!(
            &bytes[off34 as usize + 2..off34 as usize + 2 + 80],
            t34_data.as_slice()
        );
    }

    #[test]
    fn test_large_track_max_track_len() {
        let mut builder = G64Builder::new();
        // Track larger than default 7928
        let large_track = vec![0xAA; 8192];
        builder.set_track(2, 3, &large_track);

        let bytes = builder.serialize();
        let max_len = u16::from_le_bytes([bytes[10], bytes[11]]);
        assert_eq!(max_len, 8192);
    }

    #[test]
    fn test_out_of_bounds_half_track() {
        let mut builder = G64Builder::new();
        builder.set_track(84, 0, &[1, 2, 3]);
        builder.set_track(255, 0, &[1, 2, 3]);
        assert_eq!(builder.track_count(), 0);
        assert!(builder.track(84).is_none());
        assert!(builder.track(255).is_none());

        // Should not panic on clear_track out-of-bounds either
        builder.clear_track(84);
        builder.clear_track(200);
    }

    #[test]
    fn test_speed_zone_masking() {
        let mut builder = G64Builder::new();
        // Pass speed zone with high bits set (0b1111_0010 -> 2)
        builder.set_track(5, 0b1111_0010, &[0x42]);
        let (sz, _) = builder.track(5).unwrap();
        assert_eq!(sz, 2);

        let bytes = builder.serialize();
        let speed_pos = SPEED_TABLE_OFFSET + 5 * 4;
        let serialized_sz =
            u32::from_le_bytes(bytes[speed_pos..speed_pos + 4].try_into().unwrap());
        assert_eq!(serialized_sz, 2);
    }

    #[test]
    fn test_clear_track() {
        let mut builder = G64Builder::new();
        builder.set_track(10, 1, &[0x12, 0x34]);
        assert_eq!(builder.track_count(), 1);

        builder.clear_track(10);
        assert_eq!(builder.track_count(), 0);
        assert!(builder.track(10).is_none());

        let bytes = builder.serialize();
        assert_eq!(bytes.len(), TRACK_DATA_OFFSET);
    }

    #[test]
    fn test_standard_speed_zone() {
        // Tracks 1..17 (half-tracks 0..33) -> 3
        assert_eq!(standard_speed_zone(0), 3); // track 1.0
        assert_eq!(standard_speed_zone(1), 3); // track 1.5
        assert_eq!(standard_speed_zone(32), 3); // track 17.0
        assert_eq!(standard_speed_zone(33), 3); // track 17.5

        // Tracks 18..24 (half-tracks 34..47) -> 2
        assert_eq!(standard_speed_zone(34), 2); // track 18.0
        assert_eq!(standard_speed_zone(47), 2); // track 24.5

        // Tracks 25..30 (half-tracks 48..59) -> 1
        assert_eq!(standard_speed_zone(48), 1); // track 25.0
        assert_eq!(standard_speed_zone(59), 1); // track 30.5

        // Tracks 31..42 (half-tracks 60..83) -> 0
        assert_eq!(standard_speed_zone(60), 0); // track 31.0
        assert_eq!(standard_speed_zone(83), 0); // track 42.5

        // Beyond 42
        assert_eq!(standard_speed_zone(84), 0);
    }
}
