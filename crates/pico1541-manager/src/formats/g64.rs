use embedded_io_async::{Read, Seek, SeekFrom};
use pico1541_common::track_buffer::TrackLayout;

use super::{DiskImage, Error, TrackLookupTable};

pub struct G64Image<S: Read + Seek> {
    storage: S,
    lookup_table: TrackLookupTable,
}
impl<S: Read + Seek> G64Image<S> {
    pub async fn from_storage(mut storage: S) -> Result<Self, Error> {
        let mut lookup_table = TrackLookupTable::new();

        // Read the 84 track pointers
        storage
            .seek(SeekFrom::Start(12))
            .await
            .map_err(|_| Error::Io)?;
        let mut ptr_bytes = [0u8; 84 * 4];
        storage
            .read_exact(&mut ptr_bytes)
            .await
            .map_err(|_| Error::Io)?;

        for i in 0..84 {
            let offset = u32::from_le_bytes([
                ptr_bytes[i * 4],
                ptr_bytes[i * 4 + 1],
                ptr_bytes[i * 4 + 2],
                ptr_bytes[i * 4 + 3],
            ]);
            if offset != 0 {
                lookup_table.offsets[i] = Some(offset);
            }
        }

        storage
            .seek(SeekFrom::Start(0x015C))
            .await
            .map_err(|_| Error::Io)?;
        let mut speed_bytes = [0u8; 84 * 4];
        storage
            .read_exact(&mut speed_bytes)
            .await
            .map_err(|_| Error::Io)?;

        for i in 0..84 {
            let speed = u32::from_le_bytes([
                speed_bytes[i * 4],
                speed_bytes[i * 4 + 1],
                speed_bytes[i * 4 + 2],
                speed_bytes[i * 4 + 3],
            ]);
            if speed < 4 {
                lookup_table.speed_zones[i] = Some(speed as u8);
            }
        }

        Ok(Self {
            storage,
            lookup_table,
        })
    }
}
impl<S: Read + Seek> DiskImage for G64Image<S> {
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error> {
        log::info!("Reading G64 track {}", half_track);

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

        self.storage
            .seek(SeekFrom::Start(offset as u64))
            .await
            .map_err(|_| Error::Io)?;
        let mut len_bytes = [0u8; 2];
        self.storage
            .read_exact(&mut len_bytes)
            .await
            .map_err(|_| Error::Io)?;
        let track_len = u16::from_le_bytes(len_bytes) as usize;

        if track_len > dest.len() {
            return Err(Error::TrackTooLarge);
        }
        // Zero out dest buffer first
        for b in dest.iter_mut() {
            *b = 0;
        }

        self.storage
            .read_exact(&mut dest[..track_len])
            .await
            .map_err(|_| Error::Io)?;

        Ok((TrackLayout::Accurate, track_len, speed_zone))
    }
}
