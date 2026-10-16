pub mod g64;
pub mod gcr;
pub mod lz77;
pub mod nib;

pub use g64::G64Builder;
pub use gcr::{detect_cycle_and_align, sanitize_weak_bits, AlignMode};
pub use lz77::decompress;
pub use nib::{NibImage, TrackEntry};

/// Detailed report for a single converted track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackConversionReport {
    pub half_track: u8,
    pub raw_len: usize,
    pub cycle_len: usize,
    pub aligned_len: usize,
    pub speed_zone: u8,
    pub weak_bytes: usize,
}

/// Summary report of the NIB to G64 conversion process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConversionReport {
    /// Number of tracks successfully converted and written into G64.
    pub tracks_converted: usize,
    /// Total count of weak/bad GCR bytes detected (and sanitized if `keep_raw` was false).
    pub weak_bytes_sanitized: usize,
    /// Per-track conversion reports.
    pub track_reports: Vec<TrackConversionReport>,
}

/// Converts raw NIB or LZ77-compressed NBZ disk image bytes into G64 format.
///
/// # Arguments
/// - `input_bytes`: Raw byte slice of either an uncompressed NIB image or an LZ77-compressed NBZ image.
/// - `keep_raw`: If true, preserves weak bits as captured instead of replacing them with `0x00`.
/// - `align_mode`: Track alignment mode (`Auto`, `None`, or `Sync`).
///
/// # Returns
/// A tuple of `(Vec<u8>, ConversionReport)` containing the serialized G64 bytes and statistics,
/// or an error message if decompression or parsing fails.
pub fn convert_nib_to_g64(
    input_bytes: &[u8],
    keep_raw: bool,
    align_mode: AlignMode,
) -> Result<(Vec<u8>, ConversionReport), String> {
    let decompressed_buf: Vec<u8>;
    let raw_bytes: &[u8] = if input_bytes.starts_with(b"MNIB-") {
        input_bytes
    } else {
        match lz77::decompress(input_bytes) {
            Ok(decompressed) => {
                decompressed_buf = decompressed;
                &decompressed_buf
            }
            Err(e) => {
                return Err(format!(
                    "Input is not a raw NIB (does not start with MNIB-) and LZ77 decompression failed: {}",
                    e
                ));
            }
        }
    };

    let nib_image = NibImage::from_bytes(raw_bytes)?;
    let mut builder = G64Builder::new();
    let mut total_weak_bytes = 0;
    let mut tracks_converted = 0;
    let mut track_reports = Vec::new();

    for track in nib_image.tracks() {
        let (mut aligned_buf, cycle_len) =
            detect_cycle_and_align(track.data, track.speed_zone, align_mode);
        let weak_count = sanitize_weak_bits(&mut aligned_buf, keep_raw);
        builder.set_track(track.half_track, track.speed_zone, &aligned_buf);
        tracks_converted += 1;
        total_weak_bytes += weak_count;

        track_reports.push(TrackConversionReport {
            half_track: track.half_track,
            raw_len: track.data.len(),
            cycle_len,
            aligned_len: aligned_buf.len(),
            speed_zone: track.speed_zone,
            weak_bytes: weak_count,
        });
    }

    let g64_bytes = builder.serialize();
    let report = ConversionReport {
        tracks_converted,
        weak_bytes_sanitized: total_weak_bytes,
        track_reports,
    };

    Ok((g64_bytes, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_invalid_bytes() {
        let result = convert_nib_to_g64(b"bad", false, AlignMode::Auto);
        assert!(result.is_err());
    }
}
