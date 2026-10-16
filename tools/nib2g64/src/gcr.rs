//! GCR stream analysis: weak-bit detection, cycle length calculation, and alignment.

/// Alignment mode for track data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignMode {
    /// Align to sector 0 or longest inter-sector gap if CBM sectors found.
    #[default]
    Auto,
    /// Do not realign; keep the raw track start offset.
    None,
    /// Rotate track so it begins at the first SYNC mark.
    Sync,
}

/// Standard CBM GCR encoding table: nibble (4-bit) -> 5-bit GCR code.
pub const GCR_TABLE: [u8; 16] = [
    0x0A, 0x0B, 0x12, 0x13, 0x0E, 0x0F, 0x16, 0x17, 0x09, 0x19, 0x1A, 0x1B, 0x0D, 0x1D, 0x1E, 0x15,
];

/// Encodes a 4-byte chunk into 5 GCR bytes.
pub fn encode_gcr_block(src: &[u8; 4], dst: &mut [u8; 5]) {
    let mut n = [0u8; 8];
    for i in 0..4 {
        n[i * 2] = GCR_TABLE[(src[i] >> 4) as usize];
        n[i * 2 + 1] = GCR_TABLE[(src[i] & 0x0F) as usize];
    }
    dst[0] = (n[0] << 3) | (n[1] >> 2);
    dst[1] = (n[1] << 6) | (n[2] << 1) | (n[3] >> 4);
    dst[2] = (n[3] << 4) | (n[4] >> 1);
    dst[3] = (n[4] << 7) | (n[5] << 2) | (n[6] >> 3);
    dst[4] = (n[6] << 5) | n[7];
}

/// Decodes a 5-byte GCR block into 4 raw data bytes.
pub fn decode_gcr_block(src: &[u8; 5], dst: &mut [u8; 4]) {
    let mut n = [0u8; 8];
    n[0] = (src[0] >> 3) & 0x1F;
    n[1] = ((src[0] << 2) | (src[1] >> 6)) & 0x1F;
    n[2] = (src[1] >> 1) & 0x1F;
    n[3] = ((src[1] << 4) | (src[2] >> 4)) & 0x1F;
    n[4] = ((src[2] << 1) | (src[3] >> 7)) & 0x1F;
    n[5] = (src[3] >> 2) & 0x1F;
    n[6] = ((src[3] << 3) | (src[4] >> 5)) & 0x1F;
    n[7] = src[4] & 0x1F;

    for i in 0..4 {
        dst[i] = (reverse_gcr(n[i * 2]) << 4) | reverse_gcr(n[i * 2 + 1]);
    }
}

fn reverse_gcr(g: u8) -> u8 {
    match g {
        0x0A => 0,
        0x0B => 1,
        0x12 => 2,
        0x13 => 3,
        0x0E => 4,
        0x0F => 5,
        0x16 => 6,
        0x17 => 7,
        0x09 => 8,
        0x19 => 9,
        0x1A => 10,
        0x1B => 11,
        0x0D => 12,
        0x1D => 13,
        0x1E => 14,
        0x15 => 15,
        _ => 0,
    }
}

/// Checks whether a 10-bit window across `prev_byte` (last 2 bits) and `curr_byte` (all 8 bits)
/// contains 3 consecutive zero bits.
///
/// In standard GCR encoding, no valid GCR byte sequence ever has more than 2 consecutive zero bits.
/// If 3 or more consecutive zeros appear, it indicates an unformatted area or weak bits.
#[inline]
pub fn is_bad_gcr(prev_byte: u8, curr_byte: u8) -> bool {
    let window: u16 = ((prev_byte as u16 & 0x03) << 8) | (curr_byte as u16);
    for shift in 0..=7 {
        if (window & (0b111 << shift)) == 0 {
            return true;
        }
    }
    false
}

/// Scans the track buffer for bad GCR bytes.
///
/// For byte index 0, bit continuity wraps around to `track[track.len() - 1]`.
/// If `keep_raw` is false, replaces any detected bad GCR byte with `0x00`.
/// Returns the total count of bad/weak GCR bytes identified.
pub fn sanitize_weak_bits(track: &mut [u8], keep_raw: bool) -> usize {
    if track.is_empty() {
        return 0;
    }

    let len = track.len();
    let mut weak_indices = Vec::new();

    for i in 0..len {
        let prev = if i == 0 { track[len - 1] } else { track[i - 1] };
        let curr = track[i];
        if is_bad_gcr(prev, curr) {
            weak_indices.push(i);
        }
    }

    let count = weak_indices.len();
    if !keep_raw {
        for idx in weak_indices {
            track[idx] = 0x00;
        }
    }

    count
}

/// Returns nominal capacity and search window `(nominal, start..=end)` for a given speed zone.
pub fn speed_zone_window(speed_zone: u8) -> (usize, std::ops::RangeInclusive<usize>) {
    match speed_zone {
        3 => (6250, 5900..=6600),
        2 => (6666, 6300..=7000),
        1 => (7142, 6800..=7500),
        0 => (7692, 7300..=8000),
        _ => (6250, 5900..=6600),
    }
}

/// Represents an identified CBM sector header on a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectorHeader {
    /// Offset of the first sync byte (`0xFF`) preceding the header.
    pub sync_offset: usize,
    /// Offset of the `0x52` header identifier.
    pub header_offset: usize,
    /// Decoded sector ID (0..20).
    pub sector: u8,
    /// Decoded track ID (1..40).
    pub track: u8,
}

/// Locates all CBM sector headers in a track buffer.
pub fn find_sector_headers(data: &[u8]) -> Vec<SectorHeader> {
    let mut headers = Vec::new();
    let len = data.len();
    if len < 8 {
        return headers;
    }

    let mut i = 0;
    while i < len - 7 {
        if data[i] == 0xFF && data[i + 1] == 0xFF {
            let sync_start = i;
            while i < len && data[i] == 0xFF {
                i += 1;
            }
            if i + 5 <= len && data[i] == 0x52 {
                let header_offset = i;
                let mut decoded = [0u8; 4];
                let block: [u8; 5] = match data[header_offset..header_offset + 5].try_into() {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                decode_gcr_block(&block, &mut decoded);
                // decoded[0] is block type (0x08 for header), decoded[1] checksum, decoded[2] sector, decoded[3] track
                if decoded[0] == 0x08 {
                    headers.push(SectorHeader {
                        sync_offset: sync_start,
                        header_offset,
                        sector: decoded[2],
                        track: decoded[3],
                    });
                }
            }
        } else {
            i += 1;
        }
    }

    headers
}

/// Detects the physical cycle length L and aligns the track buffer.
///
/// Returns the cropped/aligned track buffer of length L and the length L itself.
pub fn detect_cycle_and_align(
    raw_track: &[u8],
    speed_zone: u8,
    align_mode: AlignMode,
) -> (Vec<u8>, usize) {
    let (nominal_len, search_range) = speed_zone_window(speed_zone);
    let raw_len = raw_track.len();

    if raw_len == 0 {
        return (Vec::new(), 0);
    }

    // Step a: CBM Sector Header Search
    let headers = find_sector_headers(raw_track);
    let mut detected_l = None;

    if !headers.is_empty() {
        // Find the first sector header near offset 0
        // Search through the earliest sector headers
        'outer: for first_hdr in headers.iter().take(4) {
            for second_hdr in headers.iter() {
                if second_hdr.header_offset > first_hdr.header_offset {
                    let dist = second_hdr.header_offset - first_hdr.header_offset;
                    if search_range.contains(&dist)
                        && second_hdr.sector == first_hdr.sector
                        && second_hdr.track == first_hdr.track
                    {
                        detected_l = Some(dist);
                        break 'outer;
                    }
                }
            }
        }
    }

    // Step b: Fallback Pattern Correlation
    if detected_l.is_none() {
        detected_l = find_cycle_by_correlation(raw_track, &search_range);
    }

    // Step c: Clamp / Fallback to nominal
    let l = detected_l.unwrap_or(nominal_len).min(raw_len);

    let l_slice = &raw_track[..l];

    // Alignment
    let aligned = match align_mode {
        AlignMode::None => l_slice.to_vec(),
        AlignMode::Auto => {
            // Find sectors in the l_slice
            let slice_headers = find_sector_headers(l_slice);
            if slice_headers.is_empty() {
                l_slice.to_vec()
            } else if let Some(sec0) = slice_headers.iter().find(|h| h.sector == 0) {
                // Align immediately before sector 0 sync mark
                rotate_buffer(l_slice, sec0.sync_offset)
            } else {
                // Find longest gap between consecutive sector sync marks
                let mut best_split = slice_headers[0].sync_offset;
                let mut max_gap = 0;
                for i in 0..slice_headers.len() {
                    let next_sync = if i + 1 < slice_headers.len() {
                        slice_headers[i + 1].sync_offset
                    } else {
                        slice_headers[0].sync_offset + l
                    };
                    let gap = next_sync.saturating_sub(slice_headers[i].header_offset + 10);
                    if gap > max_gap {
                        max_gap = gap;
                        // Put seam in the middle or start of this gap
                        best_split = if i + 1 < slice_headers.len() {
                            slice_headers[i + 1].sync_offset
                        } else {
                            slice_headers[0].sync_offset
                        };
                    }
                }
                rotate_buffer(l_slice, best_split % l)
            }
        }
        AlignMode::Sync => {
            // Rotate so it starts at the first major sync run
            if let Some(sync_pos) = find_first_sync(l_slice) {
                rotate_buffer(l_slice, sync_pos)
            } else {
                l_slice.to_vec()
            }
        }
    };

    (aligned, l)
}

fn rotate_buffer(slice: &[u8], split: usize) -> Vec<u8> {
    if slice.is_empty() || split == 0 || split >= slice.len() {
        return slice.to_vec();
    }
    let mut out = Vec::with_capacity(slice.len());
    out.extend_from_slice(&slice[split..]);
    out.extend_from_slice(&slice[..split]);
    out
}

fn find_first_sync(slice: &[u8]) -> Option<usize> {
    if slice.len() < 2 {
        return None;
    }
    (0..slice.len() - 1).find(|&i| slice[i] == 0xFF && slice[i + 1] == 0xFF)
}

/// Fallback pattern correlation: searches for repeating 16-byte signature within the search window.
fn find_cycle_by_correlation(
    raw_track: &[u8],
    search_range: &std::ops::RangeInclusive<usize>,
) -> Option<usize> {
    const SIG_LEN: usize = 16;
    let raw_len = raw_track.len();
    if raw_len < *search_range.end() + SIG_LEN {
        return None;
    }

    // Pick signature starting at first sync if available, or offset 0.
    // If offset 0 is constant noise (all same bytes), scan forward up to 500 bytes for variable pattern.
    let mut sig_start = find_first_sync(&raw_track[..500.min(raw_len)]).unwrap_or(0);
    if raw_track[sig_start..sig_start + SIG_LEN.min(raw_len - sig_start)]
        .iter()
        .all(|&b| b == raw_track[sig_start])
    {
        // Try to find a window with some byte variety in the first 500 bytes
        for candidate in 0..500.min(raw_len.saturating_sub(SIG_LEN)) {
            let slice = &raw_track[candidate..candidate + SIG_LEN];
            if !slice.iter().all(|&b| b == slice[0]) {
                sig_start = candidate;
                break;
            }
        }
    }

    if sig_start + SIG_LEN > raw_len {
        return None;
    }
    let signature = &raw_track[sig_start..sig_start + SIG_LEN];

    // If signature is still completely uniform (all same bytes, e.g. constant 0x00, 0x55, 0xAA),
    // pattern correlation cannot meaningfully find an exact rotation period.
    if signature.iter().all(|&b| b == signature[0]) {
        return None;
    }

    let mut best_offset = None;
    let mut best_score = 0;

    for offset in search_range.clone() {
        let candidate_start = sig_start + offset;
        if candidate_start + SIG_LEN <= raw_len {
            let candidate = &raw_track[candidate_start..candidate_start + SIG_LEN];
            let matches = signature
                .iter()
                .zip(candidate.iter())
                .filter(|(&a, &b)| a == b)
                .count();

            if matches > best_score {
                best_score = matches;
                best_offset = Some(offset);
            }
        }
    }

    // Require high confidence (at least 14 out of 16 bytes match)
    if best_score >= 14 {
        best_offset
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_bad_gcr() {
        // Valid GCR byte combinations (GCR codes 0x55, 0x52, 0xFF, etc.)
        assert!(!is_bad_gcr(0x55, 0x55)); // 0b01010101, 0b01010101 - no 3 consecutive zeros
        assert!(!is_bad_gcr(0xFF, 0xFF)); // all 1s
        assert!(!is_bad_gcr(0x52, 0x52)); // 0b01010010 - prev ends in 10, curr starts with 01

        // Bad GCR patterns with 3+ consecutive zeros
        assert!(is_bad_gcr(0x00, 0x00)); // all zeros
        assert!(is_bad_gcr(0xFF, 0x00)); // contains 8 consecutive zeros
        assert!(is_bad_gcr(0b00, 0b0001_1111)); // curr starts with 000
        assert!(is_bad_gcr(0b00, 0b1000_1111)); // bits 6,5,4 are 000
        assert!(is_bad_gcr(0b00, 0b1111_0001)); // bits 3,2,1 are 000
        assert!(is_bad_gcr(0b00, 0b1111_1000)); // bits 2,1,0 are 000
        // Window across prev and curr: prev last 2 bits are 00, curr starts with 0 (bit 7 = 0)
        assert!(is_bad_gcr(0b00, 0b0111_1111)); // prev ends in 00, curr bit 7 is 0 -> 3 zeros!
        // prev ends in 01, curr bit 7 is 0, bit 6 is 0 -> prev bit 0 is 1, curr 7,6 are 0 -> only 2 zeros
        assert!(!is_bad_gcr(0b01, 0b0011_1111));
        // prev ends in 00, curr bit 7 is 1 -> only 2 zeros
        assert!(!is_bad_gcr(0b00, 0b1011_1111));
    }

    #[test]
    fn test_sanitize_weak_bits() {
        let mut track = vec![0x55; 100];
        // Inject bad GCR in the middle
        track[20] = 0x00;
        track[21] = 0x00;

        let mut track_keep = track.clone();
        let weak_count = sanitize_weak_bits(&mut track_keep, true);
        assert!(weak_count >= 2);
        // Data preserved
        assert_eq!(track_keep[20], 0x00);
        assert_eq!(track_keep[21], 0x00);
        assert_eq!(track_keep[0], 0x55);

        let mut track_replace = track.clone();
        let weak_count2 = sanitize_weak_bits(&mut track_replace, false);
        assert_eq!(weak_count, weak_count2);
        assert_eq!(track_replace[20], 0x00);
        assert_eq!(track_replace[21], 0x00);

        // Test wrap-around bad GCR at index 0
        let mut wrap_track = vec![0x55; 10];
        wrap_track[9] = 0x00; // ends in 00
        wrap_track[0] = 0x3F; // starts with 00111111 -> across wrap: 00 followed by 0 = 3 zeros!
        let wrap_count = sanitize_weak_bits(&mut wrap_track, false);
        assert!(wrap_count > 0);
        assert_eq!(wrap_track[0], 0x00);
    }

    fn synthesize_cbm_sector(track: u8, sector: u8) -> Vec<u8> {
        let mut sec = Vec::new();
        // 5 sync bytes
        sec.extend_from_slice(&[0xFF; 5]);
        // Header
        let checksum = sector ^ track ^ 0x31 ^ 0x32;
        let header_raw = [0x08, checksum, sector, track, 0x32, 0x31, 0x0F, 0x0F];
        let mut gcr_hdr1 = [0u8; 5];
        let mut gcr_hdr2 = [0u8; 5];
        encode_gcr_block((&header_raw[0..4]).try_into().unwrap(), &mut gcr_hdr1);
        encode_gcr_block((&header_raw[4..8]).try_into().unwrap(), &mut gcr_hdr2);
        sec.extend_from_slice(&gcr_hdr1);
        sec.extend_from_slice(&gcr_hdr2);
        // Header gap
        sec.extend_from_slice(&[0x55; 9]);
        // Data sync
        sec.extend_from_slice(&[0xFF; 5]);
        // Data block (0x07 + 256 data + checksum + padding)
        let mut data_raw = [0x55u8; 260];
        data_raw[0] = 0x07;
        for chunk in data_raw.chunks_exact(4) {
            let mut dst = [0u8; 5];
            encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
            sec.extend_from_slice(&dst);
        }
        // Inter-sector gap
        sec.extend_from_slice(&[0x55; 8]);
        sec
    }

    #[test]
    fn test_detect_cycle_and_align_cbm_sector() {
        const REPEAT_LEN: usize = 6250;
        let mut raw = vec![0x55; 8192];

        // Synthesize repeating sectors at offset 0 and repeating at REPEAT_LEN
        let s0 = synthesize_cbm_sector(1, 0);
        let s1 = synthesize_cbm_sector(1, 1);
        let s2 = synthesize_cbm_sector(1, 2);

        // Place them at offset 100
        let mut pos = 100;
        raw[pos..pos + s0.len()].copy_from_slice(&s0);
        pos += s0.len() + 10;
        raw[pos..pos + s1.len()].copy_from_slice(&s1);
        pos += s1.len() + 10;
        raw[pos..pos + s2.len()].copy_from_slice(&s2);

        // Duplicate sector 0 at 100 + REPEAT_LEN
        let rep_pos = 100 + REPEAT_LEN;
        raw[rep_pos..rep_pos + s0.len()].copy_from_slice(&s0);

        let (aligned, l) = detect_cycle_and_align(&raw, 3, AlignMode::Auto);
        assert_eq!(l, REPEAT_LEN);
        assert_eq!(aligned.len(), REPEAT_LEN);
        // In Auto mode with Sector 0 present, aligned buffer should start at sector 0's sync
        assert_eq!(aligned[0], 0xFF);
        assert_eq!(aligned[1], 0xFF);
    }

    #[test]
    fn test_detect_cycle_and_align_correlation_fallback() {
        const REPEAT_LEN: usize = 6666;
        let mut raw = vec![0xAA; 8192];

        // Custom non-standard pattern (no CBM headers)
        let pattern: Vec<u8> = (0..64).map(|x| (x * 7) as u8).collect();
        raw[50..50 + 64].copy_from_slice(&pattern);
        raw[50 + REPEAT_LEN..50 + REPEAT_LEN + 64].copy_from_slice(&pattern);

        let (aligned, l) = detect_cycle_and_align(&raw, 2, AlignMode::None);
        assert_eq!(l, REPEAT_LEN);
        assert_eq!(aligned.len(), REPEAT_LEN);
        assert_eq!(aligned[0], raw[0]);
    }

    #[test]
    fn test_detect_cycle_and_align_nominal_fallback() {
        // Completely random or constant data with no repeated signature
        let raw = vec![0x00; 8192];
        let (aligned, l) = detect_cycle_and_align(&raw, 1, AlignMode::None);
        assert_eq!(l, 7142); // Speed zone 1 nominal
        assert_eq!(aligned.len(), 7142);
    }

    #[test]
    fn test_align_mode_sync() {
        const REPEAT_LEN: usize = 6250;
        let mut raw = vec![0x55; 8192];
        // Put sync at offset 42
        raw[42] = 0xFF;
        raw[43] = 0xFF;
        raw[44] = 0xFF;

        let (aligned, l) = detect_cycle_and_align(&raw, 3, AlignMode::Sync);
        assert_eq!(l, REPEAT_LEN);
        assert_eq!(aligned.len(), REPEAT_LEN);
        assert_eq!(aligned[0], 0xFF);
        assert_eq!(aligned[1], 0xFF);
        assert_eq!(aligned[2], 0xFF);
    }
}
