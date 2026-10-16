//! NIB (MNIB raw nibble dump) disk image parser.

/// Header size of a NIB disk image (256 bytes).
pub const HEADER_SIZE: usize = 0x100;

/// Standard track data size in bytes (8,192 bytes / 0x2000).
pub const TRACK_SIZE: usize = 8192;

/// Maximum number of track entries in a NIB header.
pub const MAX_TRACKS: usize = 84;

/// Magic identifier for 1541 raw NIB dumps.
pub const MAGIC_1541: &[u8; 13] = b"MNIB-1541-RAW";

/// Magic identifier for 1571 raw NIB dumps.
pub const MAGIC_1571: &[u8; 13] = b"MNIB-1571-RAW";

/// Represents a single track entry parsed from a NIB image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackEntry<'a> {
    /// 0-indexed half-track number (Track 1.0 = 0, Track 1.5 = 1, Track 18.0 = 34).
    pub half_track: u8,
    /// Speed zone (0..3).
    pub speed_zone: u8,
    /// 8,192 bytes of raw GCR track data.
    pub data: &'a [u8],
}

/// A parsed NIB disk image referencing the underlying byte buffer.
#[derive(Debug, Clone)]
pub struct NibImage<'a> {
    tracks: Vec<TrackEntry<'a>>,
}

impl<'a> NibImage<'a> {
    /// Parses a NIB image from raw file bytes.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<NibImage<'a>, String> {
        if bytes.len() < HEADER_SIZE {
            return Err(format!(
                "File too small for NIB header: expected at least {} bytes, got {}",
                HEADER_SIZE,
                bytes.len()
            ));
        }

        let magic = &bytes[0..13];
        if magic != MAGIC_1541 && magic != MAGIC_1571 {
            return Err(format!(
                "Invalid NIB magic signature: {:?}",
                String::from_utf8_lossy(magic)
            ));
        }

        let mut tracks = Vec::new();
        for (entry_index, &[track_num, density]) in bytes[0x10..HEADER_SIZE]
            .as_chunks::<2>()
            .0
            .iter()
            .take(MAX_TRACKS)
            .enumerate()
        {

            if track_num == 0 {
                break;
            }

            if track_num < 2 {
                return Err(format!(
                    "Invalid track number {} at table index {} (must be >= 2)",
                    track_num, entry_index
                ));
            }

            let half_track = track_num - 2;
            let speed_zone = density & 0x03;
            let offset = HEADER_SIZE + entry_index * TRACK_SIZE;

            if offset + TRACK_SIZE > bytes.len() {
                return Err(format!(
                    "NIB image truncated: track entry {} (half-track {}) requires offset {}..{}, but file length is {}",
                    entry_index,
                    half_track,
                    offset,
                    offset + TRACK_SIZE,
                    bytes.len()
                ));
            }

            let data = &bytes[offset..offset + TRACK_SIZE];
            tracks.push(TrackEntry {
                half_track,
                speed_zone,
                data,
            });
        }

        Ok(NibImage { tracks })
    }

    /// Returns a slice of all valid track entries in the NIB image.
    pub fn tracks(&self) -> &[TrackEntry<'a>] {
        &self.tracks
    }

    /// Looks up a track entry by its 0-indexed half-track number.
    pub fn track(&self, half_track: u8) -> Option<TrackEntry<'a>> {
        self.tracks.iter().find(|t| t.half_track == half_track).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_nib(magic: &[u8; 13], entries: &[(u8, u8)]) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE + entries.len() * TRACK_SIZE];
        buf[0..13].copy_from_slice(magic);
        for (i, &(track_num, density)) in entries.iter().enumerate() {
            let table_offset = 0x10 + i * 2;
            buf[table_offset] = track_num;
            buf[table_offset + 1] = density;

            let data_offset = HEADER_SIZE + i * TRACK_SIZE;
            // Fill with a recognizable byte per track
            buf[data_offset..data_offset + TRACK_SIZE].fill(track_num);
        }
        buf
    }

    #[test]
    fn test_valid_1541_nib_parsing() {
        // Track 1.0 (track_num 2, speed zone 3), Track 18.0 (track_num 36, speed zone 1)
        let entries = vec![(2u8, 0x03u8), (36u8, 0x01u8)];
        let data = create_test_nib(MAGIC_1541, &entries);

        let image = NibImage::from_bytes(&data).expect("parsing should succeed");
        assert_eq!(image.tracks().len(), 2);

        let t0 = image.track(0).expect("half-track 0 should exist");
        assert_eq!(t0.half_track, 0);
        assert_eq!(t0.speed_zone, 3);
        assert_eq!(t0.data.len(), TRACK_SIZE);
        assert_eq!(t0.data[0], 2);

        let t34 = image.track(34).expect("half-track 34 should exist");
        assert_eq!(t34.half_track, 34);
        assert_eq!(t34.speed_zone, 1);
        assert_eq!(t34.data.len(), TRACK_SIZE);
        assert_eq!(t34.data[0], 36);

        // Missing half track
        assert!(image.track(1).is_none());
    }

    #[test]
    fn test_valid_1571_nib_parsing() {
        let entries = vec![(2u8, 0x02u8)];
        let data = create_test_nib(MAGIC_1571, &entries);

        let image = NibImage::from_bytes(&data).expect("1571 parsing should succeed");
        assert_eq!(image.tracks().len(), 1);
        assert_eq!(image.tracks()[0].half_track, 0);
        assert_eq!(image.tracks()[0].speed_zone, 2);
    }

    #[test]
    fn test_density_speed_zone_masking() {
        // Density with custom flags set: 0b1101_0010 -> speed_zone should be 2
        let entries = vec![(2u8, 0xD2u8)];
        let data = create_test_nib(MAGIC_1541, &entries);

        let image = NibImage::from_bytes(&data).expect("parsing should succeed");
        assert_eq!(image.tracks()[0].speed_zone, 2);
    }

    #[test]
    fn test_stop_on_track_zero() {
        let mut buf = vec![0u8; HEADER_SIZE + 2 * TRACK_SIZE];
        buf[0..13].copy_from_slice(MAGIC_1541);
        // Entry 0: track_num = 2
        buf[0x10] = 2;
        buf[0x11] = 0;
        // Entry 1: track_num = 0 (terminator)
        buf[0x12] = 0;
        buf[0x13] = 0;
        // Entry 2: track_num = 4 (should be ignored)
        buf[0x14] = 4;
        buf[0x15] = 0;

        let image = NibImage::from_bytes(&buf).expect("parsing should succeed");
        assert_eq!(image.tracks().len(), 1);
        assert_eq!(image.tracks()[0].half_track, 0);
    }

    #[test]
    fn test_cap_at_84_entries() {
        // Create 85 entries
        let mut entries = Vec::new();
        for i in 0..85 {
            entries.push(((i + 2) as u8, 0u8));
        }
        let data = create_test_nib(MAGIC_1541, &entries);

        let image = NibImage::from_bytes(&data).expect("parsing should succeed");
        assert_eq!(image.tracks().len(), MAX_TRACKS);
        assert_eq!(MAX_TRACKS, 84);
    }

    #[test]
    fn test_error_too_small() {
        let data = vec![0u8; 100];
        let err = NibImage::from_bytes(&data).unwrap_err();
        assert!(err.contains("File too small for NIB header"));
    }

    #[test]
    fn test_error_invalid_magic() {
        let mut data = vec![0u8; HEADER_SIZE];
        data[0..13].copy_from_slice(b"INVALID-MAGIC");
        let err = NibImage::from_bytes(&data).unwrap_err();
        assert!(err.contains("Invalid NIB magic signature"));
    }

    #[test]
    fn test_error_track_num_less_than_two() {
        let mut data = vec![0u8; HEADER_SIZE + TRACK_SIZE];
        data[0..13].copy_from_slice(MAGIC_1541);
        data[0x10] = 1; // track_num 1 is invalid
        data[0x11] = 0;
        let err = NibImage::from_bytes(&data).unwrap_err();
        assert!(err.contains("Invalid track number 1"));
    }

    #[test]
    fn test_error_truncated_data() {
        // Header declares 1 track, but file only has 256 bytes
        let mut data = vec![0u8; HEADER_SIZE];
        data[0..13].copy_from_slice(MAGIC_1541);
        data[0x10] = 2;
        data[0x11] = 0;
        let err = NibImage::from_bytes(&data).unwrap_err();
        assert!(err.contains("NIB image truncated"));
    }
}
