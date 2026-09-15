use crate::gcr;
use embedded_io_async::{Read, Seek, SeekFrom};
use pico1541_common::track_buffer::TrackLayout;

use super::{sectors_per_track, DiskImage, Error, TrackLookupTable};

pub struct D64Image<S: Read + Seek> {
    storage: S,
    id: [u8; 2],
    lookup_table: TrackLookupTable,
}

impl<S: Read + Seek> D64Image<S> {
    pub fn new(storage: S) -> Self {
        let mut lookup_table = TrackLookupTable::new();
        for half_track in 0..=84 {
            if half_track % 2 == 0 {
                let track = half_track / 2 + 1;
                lookup_table.offsets[half_track as usize] = Some(Self::get_track_offset(track));
            }
        }

        Self {
            storage,
            id: [0x31, 0x31],
            lookup_table,
        }
    }
    pub fn set_id(&mut self, id: [u8; 2]) {
        self.id = id;
    }

    async fn read_sector_data(&mut self, offset: u64, dest: &mut [u8]) -> Result<(), Error> {
        self.storage
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|_| Error::Io)?;
        self.storage.read_exact(dest).await.map_err(|_| Error::Io)?;
        Ok(())
    }

    fn get_track_offset(track: u8) -> u32 {
        let mut offset = 0u32;
        for t in 1..track {
            offset += (sectors_per_track(t) as u32) * 256;
        }
        offset
    }
}

impl<S: Read + Seek> DiskImage for D64Image<S> {
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

        let track = half_track / 2 + 1;
        let offset = track_offset.unwrap();
        if !(1..=40).contains(&track) {
            return Err(Error::InvalidTrack);
        }
        let sectors = sectors_per_track(track);
        let speed_zone = pico1541_common::track_buffer::default_speed_zone(
            pico1541_common::track_buffer::track_to_half_track(track),
        );
        let speed_zone_len: usize = pico1541_common::track_buffer::speed_zone_capacity(speed_zone);
        let base_sector_len = 5 + 10 + 9 + 5 + 325; // 5 sync + 10 header + 9 gap + 5 sync + 325 data
        let total_base_len = sectors as usize * base_sector_len;
        let remaining_gap = speed_zone_len.saturating_sub(total_base_len);
        let gap_per_sector = remaining_gap / sectors as usize;

        let mut write_ptr = 0;
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0x55;
                write_ptr += 1;
            }
        }

        for logical_sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            let sector = logical_sector;
            let sector_offset = sector as usize * 256;
            self.read_sector_data(offset as u64 + sector_offset as u64, &mut sector_data)
                .await?;

            if track == 18 && sector == 0 {
                self.id[0] = sector_data[162];
                self.id[1] = sector_data[163];
            }

            for _ in 0..5 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            let checksum = sector ^ track ^ self.id[0] ^ self.id[1];
            let header_raw = [
                0x08, checksum, sector, track, self.id[1], self.id[0], 0x0F, 0x0F,
            ];
            for chunk in header_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            for _ in 0..9 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }
            for _ in 0..5 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            let mut data_checksum = 0u8;
            for b in sector_data.iter() {
                data_checksum ^= b;
            }
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            data_raw[1..257].copy_from_slice(&sector_data);
            data_raw[257] = data_checksum;
            for chunk in data_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            for _ in 0..gap_per_sector {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }
        }
        while write_ptr < speed_zone_len && write_ptr < dest.len() {
            dest[write_ptr] = 0x55;
            write_ptr += 1;
        }
        let track_bytes = write_ptr;
        Ok((TrackLayout::Accurate, track_bytes, None))
    }
}
