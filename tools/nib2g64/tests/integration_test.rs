use std::fs;
use std::path::Path;

use nib2g64::g64::{OFFSET_TABLE_OFFSET, SPEED_TABLE_OFFSET};
use nib2g64::nib::{HEADER_SIZE, MAGIC_1541, TRACK_SIZE};
use nib2g64::{convert_nib_to_g64, AlignMode};

/// Creates a mock raw NIB image buffer with given track entries: `(track_num, density, fill_byte)`.
fn create_mock_nib(entries: &[(u8, u8, u8)]) -> Vec<u8> {
    let mut buf = vec![0u8; HEADER_SIZE + entries.len() * TRACK_SIZE];
    buf[0..13].copy_from_slice(MAGIC_1541);

    for (i, &(track_num, density, fill_byte)) in entries.iter().enumerate() {
        let table_offset = 0x10 + i * 2;
        buf[table_offset] = track_num;
        buf[table_offset + 1] = density;

        let data_offset = HEADER_SIZE + i * TRACK_SIZE;
        buf[data_offset..data_offset + TRACK_SIZE].fill(fill_byte);
    }

    buf
}

#[test]
fn test_synthetic_nib_roundtrip_conversion() {
    // Track 1.0 (track_num 2 -> half_track 0, speed_zone 3, fill 0x55)
    // Track 18.0 (track_num 36 -> half_track 34, speed_zone 1, fill 0xAA)
    // 0x55 (01010101) and 0xAA (10101010) contain no weak bit sequences (>2 zeros).
    let entries = vec![(2u8, 0x03u8, 0x55u8), (36u8, 0x01u8, 0xAAu8)];
    let nib_bytes = create_mock_nib(&entries);

    let (g64_bytes, report) =
        convert_nib_to_g64(&nib_bytes, false, AlignMode::None)
            .expect("synthetic NIB conversion failed");

    assert_eq!(report.tracks_converted, 2);
    assert_eq!(report.track_reports.len(), 2);
    assert_eq!(report.track_reports[0].half_track, 0);
    assert_eq!(report.track_reports[0].speed_zone, 3);
    assert_eq!(report.track_reports[1].half_track, 34);
    assert_eq!(report.track_reports[1].speed_zone, 1);

    // G64 Header validation
    assert!(g64_bytes.len() >= 684);
    assert_eq!(&g64_bytes[0..8], b"GCR-1541");
    assert_eq!(g64_bytes[8], 0x00); // version
    assert_eq!(g64_bytes[9], 84); // max half-tracks

    // Verify track 1.0 (half-track 0) offset and speed zone
    let offset_t0 = u32::from_le_bytes(
        g64_bytes[OFFSET_TABLE_OFFSET..OFFSET_TABLE_OFFSET + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let speed_t0 = u32::from_le_bytes(
        g64_bytes[SPEED_TABLE_OFFSET..SPEED_TABLE_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    assert!(offset_t0 >= 684);
    assert_eq!(speed_t0, 3);

    // Verify track 18.0 (half-track 34) offset and speed zone
    let ht34_off_pos = OFFSET_TABLE_OFFSET + 34 * 4;
    let offset_t34 = u32::from_le_bytes(
        g64_bytes[ht34_off_pos..ht34_off_pos + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let ht34_spd_pos = SPEED_TABLE_OFFSET + 34 * 4;
    let speed_t34 = u32::from_le_bytes(
        g64_bytes[ht34_spd_pos..ht34_spd_pos + 4]
            .try_into()
            .unwrap(),
    );
    assert!(offset_t34 > offset_t0);
    assert_eq!(speed_t34, 1);

    // Verify unformatted half-track 1 has offset 0
    let ht1_off_pos = OFFSET_TABLE_OFFSET + 1 * 4;
    let offset_t1 = u32::from_le_bytes(
        g64_bytes[ht1_off_pos..ht1_off_pos + 4]
            .try_into()
            .unwrap(),
    );
    assert_eq!(offset_t1, 0);

    // Verify track 1.0 payload
    let t0_len = u16::from_le_bytes(
        g64_bytes[offset_t0..offset_t0 + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    assert!(t0_len > 0);
    let t0_payload = &g64_bytes[offset_t0 + 2..offset_t0 + 2 + t0_len];
    assert!(t0_payload.iter().all(|&b| b == 0x55));

    // Verify track 18.0 payload
    let t34_len = u16::from_le_bytes(
        g64_bytes[offset_t34..offset_t34 + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    assert!(t34_len > 0);
    let t34_payload = &g64_bytes[offset_t34 + 2..offset_t34 + 2 + t34_len];
    assert!(t34_payload.iter().all(|&b| b == 0xAA));
}

fn find_fixture(rel: &str) -> Option<std::path::PathBuf> {
    let p1 = Path::new(rel);
    if p1.exists() {
        return Some(p1.to_path_buf());
    }
    let p2 = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../").join(rel);
    if p2.exists() {
        return Some(p2);
    }
    None
}

#[test]
fn test_synthetic_weak_bit_sanitization() {
    // Create a track containing 0x00 bytes, which are invalid GCR weak bits
    let entries = vec![(2u8, 0x03u8, 0x00u8)];
    let nib_bytes = create_mock_nib(&entries);

    // Convert with keep_raw = false (should replace with 0x00 and count them)
    let (g64_sanitized, report_sanitized) =
        convert_nib_to_g64(&nib_bytes, false, AlignMode::None)
            .expect("conversion should succeed");
    assert_eq!(report_sanitized.tracks_converted, 1);
    assert!(report_sanitized.weak_bytes_sanitized > 0);

    // Convert with keep_raw = true (should report weak bytes but preserve raw bytes)
    let (g64_raw, report_raw) =
        convert_nib_to_g64(&nib_bytes, true, AlignMode::None)
            .expect("conversion should succeed");
    assert_eq!(report_raw.tracks_converted, 1);
    assert_eq!(
        report_raw.weak_bytes_sanitized,
        report_sanitized.weak_bytes_sanitized
    );
    assert_eq!(g64_sanitized.len(), g64_raw.len());
}

#[test]
fn test_arkanoid_nib_conversion() {
    let path = find_fixture("temp/arkanoid.nib").expect("temp/arkanoid.nib fixture not found");
    let nib_bytes = fs::read(&path).expect("failed to read temp/arkanoid.nib");
    let (g64_bytes, report) =
        convert_nib_to_g64(&nib_bytes, false, AlignMode::Auto)
            .expect("conversion of arkanoid.nib failed");

    // Arkanoid contains 41 full tracks (tracks 1 to 41, half-tracks 0, 2, ..., 80)
    assert!(
        report.tracks_converted >= 40,
        "expected at least 40 tracks converted, got {}",
        report.tracks_converted
    );

    // Resulting G64 file must be > 200 KB
    assert!(
        g64_bytes.len() > 200 * 1024,
        "expected G64 file > 200 KB, got {} bytes",
        g64_bytes.len()
    );

    // Valid G64 header
    assert_eq!(&g64_bytes[0..8], b"GCR-1541");
    assert_eq!(g64_bytes[8], 0x00);
    assert_eq!(g64_bytes[9], 84);

    // Valid offset tables
    let offset_t0 = u32::from_le_bytes(
        g64_bytes[OFFSET_TABLE_OFFSET..OFFSET_TABLE_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    assert!(offset_t0 >= 684, "half-track 0 offset must be >= 684");

    // Track 41 is half-track 80
    let ht80_pos = OFFSET_TABLE_OFFSET + 80 * 4;
    let offset_t80 = u32::from_le_bytes(
        g64_bytes[ht80_pos..ht80_pos + 4]
            .try_into()
            .unwrap(),
    );
    assert!(
        offset_t80 > offset_t0,
        "half-track 80 offset must be greater than half-track 0 offset"
    );

    // Weak bits were detected and sanitized
    assert!(
        report.weak_bytes_sanitized > 0,
        "expected weak bits to be detected and sanitized, got 0"
    );
}

#[test]
fn test_arkanoid_nbz_conversion() {
    let path = find_fixture("temp/arkanoid.nbz").expect("temp/arkanoid.nbz fixture not found");
    let nbz_bytes = fs::read(&path).expect("failed to read temp/arkanoid.nbz");
    let (g64_bytes, report) =
        convert_nib_to_g64(&nbz_bytes, false, AlignMode::Auto)
            .expect("conversion of arkanoid.nbz failed");

    assert!(report.tracks_converted >= 40);
    assert!(g64_bytes.len() > 200 * 1024);
    assert_eq!(&g64_bytes[0..8], b"GCR-1541");
    assert!(report.weak_bytes_sanitized > 0);
}
