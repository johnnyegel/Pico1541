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

        if dest.len() < 8192 {
            return Err(Error::TrackTooLarge);
        }

        self.storage
            .seek(SeekFrom::Start(offset as u64))
            .await
            .map_err(|_| Error::Io)?;

        self.storage
            .read_exact(&mut dest[..8192])
            .await
            .map_err(|_| Error::Io)?;

        Ok((TrackLayout::Accurate, 8192, speed_zone))
    }
}
