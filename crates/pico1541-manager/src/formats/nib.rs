use embedded_io_async::{Read, Seek, SeekFrom};
use pico1541_common::track_buffer::TrackLayout;

use super::{DiskImage, Error, TrackLookupTable};

pub struct NibImage<S: Read + Seek> {
    storage: S,
    lookup_table: TrackLookupTable,
}

impl<S: Read + Seek> NibImage<S> {
    pub async fn from_storage(mut storage: S) -> Result<Self, Error> {
        let mut lookup_table = TrackLookupTable::new();

        storage
            .seek(SeekFrom::Start(0))
            .await
            .map_err(|_| Error::Io)?;
        let mut header = [0u8; 0x100];
        storage
            .read_exact(&mut header)
            .await
            .map_err(|_| Error::Io)?;

        let magic = &header[0..13];
        if magic != b"MNIB-1541-RAW" && magic != b"MNIB-1571-RAW" {
            return Err(Error::Corrupted);
        }

        // Parse track table starting at 0x10
        let mut h_index = 0x10;
        let mut t_index = 0;
        while h_index + 1 < 0x100 && header[h_index] != 0 && t_index < 84 {
            let track_num = header[h_index];
            let density = header[h_index + 1];

            // In NIB format, track 1.0 is 2, track 18.0 is 36.
            // Pico1541 0-indexed half-track = track_num - 2.
            if track_num >= 2 {
                let half_track = (track_num - 2) as usize;
                if half_track < lookup_table.offsets.len() {
                    lookup_table.offsets[half_track] = Some(0x100 + (t_index * 8192));
                    lookup_table.speed_zones[half_track] = Some(density & 0x03);
                }
            }

            h_index += 2;
            t_index += 1;
        }

        // Precalculate track lengths at mount time into the lookup table
        let mut head_buf = [0u8; 128];
        let mut win_buf = [0u8; 1024];

        for half_track in 0..lookup_table.offsets.len() {
            if let (Some(offset), Some(zone)) = (
                lookup_table.offsets[half_track],
                lookup_table.speed_zones[half_track],
            ) {
                let len = match probe_track_length(&mut storage, offset, zone, &mut head_buf, &mut win_buf).await {
                    Ok(l) => l,
                    Err(_) => pico1541_common::track_buffer::speed_zone_capacity(zone) as u16,
                };
                lookup_table.lengths[half_track] = Some(len);
            }
        }

        Ok(Self {
            storage,
            lookup_table,
        })
    }
}

impl<S: Read + Seek> DiskImage for NibImage<S> {
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error> {
        let track_offset = self
            .lookup_table
            .offsets
            .get(half_track as usize)
            .copied()
            .flatten();

        if track_offset.is_none() {
            return Ok((TrackLayout::Void, 0, None));
        }

        let offset = track_offset.unwrap();
        let speed_zone = self
            .lookup_table
            .speed_zones
            .get(half_track as usize)
            .copied()
            .flatten();
        let track_len = self
            .lookup_table
            .lengths
            .get(half_track as usize)
            .copied()
            .flatten()
            .unwrap_or(8192) as usize;

        if track_len > dest.len() {
            return Err(Error::TrackTooLarge);
        }

        self.storage
            .seek(SeekFrom::Start(offset as u64))
            .await
            .map_err(|_| Error::Io)?;

        self.storage
            .read_exact(&mut dest[..track_len])
            .await
            .map_err(|_| Error::Io)?;

        Ok((TrackLayout::Accurate, track_len, speed_zone))
    }
}

/// Probes a single NIB track to determine its true revolution length.
///
/// Because NIB tracks contain ~1.1 to 1.3 revolutions (~8192 bytes), the trailing
/// bytes repeat earlier data. This function reads a small 128-byte probe from the
/// track head and searches for its match in a 1024-byte window around the nominal
/// capacity.
async fn probe_track_length<S: Read + Seek>(
    storage: &mut S,
    offset: u32,
    speed_zone: u8,
    head_buf: &mut [u8; 128],
    win_buf: &mut [u8; 1024],
) -> Result<u16, Error> {
    let nominal = pico1541_common::track_buffer::speed_zone_capacity(speed_zone) as u32;

    // 1. Read first 128 bytes of track
    storage
        .seek(SeekFrom::Start(offset as u64))
        .await
        .map_err(|_| Error::Io)?;
    storage.read_exact(head_buf).await.map_err(|_| Error::Io)?;

    // 2. Read search window around nominal capacity
    let win_start = offset + nominal.saturating_sub(500);
    let win_len = 1000.min(8192usize.saturating_sub((win_start - offset) as usize));
    storage
        .seek(SeekFrom::Start(win_start as u64))
        .await
        .map_err(|_| Error::Io)?;
    storage
        .read_exact(&mut win_buf[..win_len])
        .await
        .map_err(|_| Error::Io)?;

    // 3. Header-based match: search for sync (>= 2 bytes 0xFF) followed by 0x52 in head_buf
    let mut i = 0;
    while i + 12 <= head_buf.len() {
        if head_buf[i] == 0xFF && head_buf[i + 1] == 0xFF {
            while i < head_buf.len() && head_buf[i] == 0xFF {
                i += 1;
            }
            if i + 10 <= head_buf.len() && head_buf[i] == 0x52 {
                let hdr = &head_buf[i..i + 10];
                let mut j = 0;
                while j + 12 <= win_len {
                    if win_buf[j] == 0xFF && win_buf[j + 1] == 0xFF {
                        while j < win_len && win_buf[j] == 0xFF {
                            j += 1;
                        }
                        if j + 10 <= win_len && &win_buf[j..j + 10] == hdr {
                            let dist = (win_start - offset) + j as u32 - i as u32;
                            return Ok(dist as u16);
                        }
                    }
                    j += 1;
                }
            }
        }
        i += 1;
    }

    // 4. Raw pattern match: find an 8-byte non-repeating probe in head_buf
    for p in (0..head_buf.len().saturating_sub(16)).step_by(8) {
        let probe = &head_buf[p..p + 8];
        if probe.iter().any(|&b| b == 0xFF || b == 0x00) {
            continue;
        }
        if probe[1..].iter().all(|&b| b == probe[0]) {
            continue;
        }

        for j in 0..win_len.saturating_sub(16) {
            if &win_buf[j..j + 8] == probe {
                if win_buf[j..j + 16] == head_buf[p..p + 16] {
                    let dist = (win_start - offset) + j as u32 - p as u32;
                    return Ok(dist as u16);
                }
            }
        }
    }

    // 5. Fallback to standard nominal capacity
    Ok(nominal as u16)
}
