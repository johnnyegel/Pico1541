/// Standard D64 file sizes: 35-track, 35-track+errors, 40-track, 40-track+errors.
pub const D64_VALID_SIZES: &[usize] = &[174_848, 175_531, 196_608, 197_376];

/// Byte offset of track 18 (BAM sector) in a D64 file.
/// Tracks 1-17: 21 sectors × 256 bytes = 91 392 bytes.
pub const D64_TRACK18_OFFSET: usize = 91_392;

/// G64 file header magic bytes.
pub const G64_MAGIC: &[u8; 8] = b"GCR-1541";

/// Information extracted from a D64 BAM sector without decoding GCR.
pub struct D64Info {
    /// True if the file size matches a known valid D64 layout.
    pub valid_size: bool,
    /// Disk ID bytes (BAM sector offset 162–163).
    pub disk_id: [u8; 2],
    /// Disk name as raw PETSCII (BAM sector offset 144–159, 0xA0-padded).
    /// Use [`petscii_to_ascii`] to render it.
    pub disk_name_petscii: [u8; 16],
}

/// Extract disk metadata from a raw D64 byte slice.
/// Returns `None` if the slice is too small to contain the BAM sector.
pub fn inspect_d64(data: &[u8]) -> Option<D64Info> {
    if data.len() < D64_TRACK18_OFFSET + 164 {
        return None;
    }
    let bam = &data[D64_TRACK18_OFFSET..];
    let mut disk_name_petscii = [0xA0u8; 16];
    disk_name_petscii.copy_from_slice(&bam[144..160]);
    Some(D64Info {
        valid_size: D64_VALID_SIZES.contains(&data.len()),
        disk_id: [bam[162], bam[163]],
        disk_name_petscii,
    })
}

/// Convert PETSCII bytes (0xA0-padded) to printable ASCII into `dst`.
/// Returns the number of significant bytes written (trailing spaces stripped).
/// Safe to call in `no_std` — writes directly into a caller-supplied buffer.
pub fn petscii_to_ascii(src: &[u8], dst: &mut [u8]) -> usize {
    let len = src.len().min(dst.len());
    for i in 0..len {
        dst[i] = if src[i] == 0xA0 { b' ' } else { src[i].clamp(0x20, 0x7E) };
    }
    let mut end = len;
    while end > 0 && dst[end - 1] == b' ' {
        end -= 1;
    }
    end
}
