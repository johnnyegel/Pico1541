use crate::gcr;
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;

use super::{sectors_per_track, DiskImage, Error};

pub struct VirtualDiskImage<S, D: DirectorySource> {
    pub source: D,
    pub file_storage: Option<S>,
    id: [u8; 2],
    pub shortcut_entry: Option<crate::storage::DirectoryEntry>,
    pub prg_start_track: u8,
    pub prg_start_sector: u8,
}

impl<S: Read + Seek, D: DirectorySource> VirtualDiskImage<S, D> {
    pub fn new(source: D, name: &[u8]) -> Self {
        // Generate a pseudo-random ID based on the disk name, ensuring valid uppercase PETSCII (0x41-0x5A)
        let mut hash1 = 0u32;
        let mut hash2 = 0u32;
        for (i, &b) in name.iter().enumerate() {
            if i % 2 == 0 {
                hash1 = hash1.wrapping_add(b as u32);
            } else {
                hash2 = hash2.wrapping_add(b as u32);
            }
        }
        let id_high = 0x41 + (hash1 % 26) as u8;
        let id_low = 0x41 + (hash2 % 26) as u8;

        Self {
            source,
            file_storage: None,
            id: [id_high, id_low],
            shortcut_entry: None,
            prg_start_track: 17,
            prg_start_sector: 0,
        }
    }

    pub fn set_prg_start(&mut self, track: u8, sector: u8) {
        self.prg_start_track = track;
        self.prg_start_sector = sector;
    }

    pub fn set_id(&mut self, id: [u8; 2]) {
        self.id = id;
    }

    pub fn set_shortcut_entry(&mut self, entry: Option<crate::storage::DirectoryEntry>) {
        self.shortcut_entry = entry;
    }

    pub fn encode_directory_sector(
        &self,
        entries: &[crate::storage::DirectoryEntry],
        next_track: u8,
        next_sector: u8,
        dest: &mut [u8],
    ) {
        let mut raw = [0u8; 256];
        raw[0] = next_track;
        raw[1] = next_sector;

        for (i, entry) in entries.iter().enumerate() {
            let offset = 2 + i * 32;
            if offset >= 256 {
                break;
            }
            raw[offset] = entry.file_type;
            if offset + 1 < 256 {
                raw[offset + 1] = entry.first_track;
            }
            if offset + 2 < 256 {
                raw[offset + 2] = entry.first_sector;
            }
            for j in 0..16 {
                if offset + 3 + j < 256 {
                    raw[offset + 3 + j] = entry.filename[j];
                }
            }
            if offset + 19 < 256 {
                raw[offset + 19] = 0;
            } // side track
            if offset + 20 < 256 {
                raw[offset + 20] = 0;
            } // side sector
            if offset + 21 < 256 {
                raw[offset + 21] = 0;
            } // REL record length
            if offset + 28 < 256 {
                raw[offset + 28] = (entry.block_size & 0xFF) as u8;
            }
            if offset + 29 < 256 {
                raw[offset + 29] = (entry.block_size >> 8) as u8;
            }
        }
        let len = dest.len().min(256);
        dest[..len].copy_from_slice(&raw[..len]);
    }

    pub async fn force_update_directory_sectors(&mut self, gcr_data: &mut [u8]) {
        let _ = self.read_track(34, gcr_data).await;
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for VirtualDiskImage<S, D> {
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error> {
        log::info!("Reading VirtualDisk track {}", half_track);

        if !half_track.is_multiple_of(2) || half_track > 84 {
            return Ok((TrackLayout::Void, 0, None));
        }

        let track = half_track / 2 + 1;

        if track <= 17 && self.file_storage.is_some() {
            if track > self.prg_start_track {
                return Ok((TrackLayout::Void, 0, None));
            }

            // PRG File Tracks: Lay out the PRG data across tracks prg_start_track down to 1.
            let sectors = 21;
            let speed_zone = pico1541_common::track_buffer::default_speed_zone(half_track);
            let speed_zone_len: usize =
                pico1541_common::track_buffer::speed_zone_capacity(speed_zone);
            let base_sector_len = 5 + 10 + 9 + 5 + 325; // 354
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

            // Track index relative to prg_start_track downwards
            let t_idx = (self.prg_start_track - track) as u64;
            let track_byte_offset = t_idx * 21 * 254;
            let start_sec = if track == self.prg_start_track {
                self.prg_start_sector
            } else {
                0
            };

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;

                // Chain step for standard interleave 10:
                let step = ((physical_sector + 21 - start_sec as usize) * 19) % 21;
                let file_pos = track_byte_offset + (step as u64 * 254);

                let mut sector_data = [0u8; 256];
                if let Some(storage) = &mut self.file_storage {
                    if storage
                        .seek(embedded_io_async::SeekFrom::Start(file_pos))
                        .await
                        .is_ok()
                    {
                        let mut file_buf = [0u8; 254];
                        let mut bytes_read = 0;
                        loop {
                            let n = storage.read(&mut file_buf[bytes_read..]).await.unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            bytes_read += n;
                            if bytes_read == 254 {
                                break;
                            }
                        }

                        if bytes_read > 0 {
                            sector_data[2..2 + bytes_read].copy_from_slice(&file_buf[..bytes_read]);

                            if bytes_read == 254 {
                                // Check if there is more file data after this block
                                let mut peek = [0u8; 1];
                                let has_more = storage.read(&mut peek).await.unwrap_or(0) > 0;

                                if has_more {
                                    if step + 1 < 21 {
                                        sector_data[0] = track;
                                        sector_data[1] =
                                            ((start_sec as usize + (step + 1) * 10) % 21) as u8;
                                    } else if track > 1 {
                                        sector_data[0] = track - 1;
                                        sector_data[1] = 0;
                                    } else {
                                        // wrap to prg_start_track
                                        sector_data[0] = self.prg_start_track;
                                        sector_data[1] = 0;
                                    }
                                } else {
                                    // Exactly 254 bytes on the last block
                                    sector_data[0] = 0;
                                    sector_data[1] = 255;
                                }
                            } else {
                                // End of file (short block)
                                sector_data[0] = 0;
                                sector_data[1] = (bytes_read + 1) as u8;
                            }
                        }
                    }
                }

                // Header SYNC (5 bytes)
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
                    crate::gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
                // Header-to-Data Gap (9 bytes)
                for _ in 0..9 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0x55;
                        write_ptr += 1;
                    }
                }
                // Data SYNC (5 bytes)
                for _ in 0..5 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xFF;
                        write_ptr += 1;
                    }
                }
                // Data Block (260 bytes GCR)
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
                    crate::gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                    if write_ptr + 5 <= dest.len() {
                        dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                        write_ptr += 5;
                    }
                }
                // Inter-sector gap
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

            log::info!(
                "VirtualDiskImage: Generated PRG track {} ({} bytes)",
                track,
                write_ptr
            );
            return Ok((TrackLayout::Accurate, write_ptr, None));
        }

        // Track 18: BAM and directory track
        if track == 18 {
            // Track 18: Generate BAM and initial directory sectors
            let sectors = 19;
            let speed_zone = pico1541_common::track_buffer::default_speed_zone(
                pico1541_common::track_buffer::track_to_half_track(18),
            );
            let speed_zone_len: usize =
                pico1541_common::track_buffer::speed_zone_capacity(speed_zone);
            let base_sector_len = 5 + 10 + 9 + 5 + 325;
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

            self.source.rewind();

            let mut peek_slot: Option<crate::storage::DirectoryEntry> = None;

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;
                let mut sector_data = [0u8; 256];

                if sector == 0 {
                    sector_data[0] = 18;
                    sector_data[1] = 1;
                    sector_data[2] = 0x41;
                    sector_data[3] = 0x00;
                    // Format BAM entries for tracks 1..35
                    for t in 1..=35u8 {
                        let base = 4 + (t as usize - 1) * 4;
                        if t == 18 {
                            sector_data[base] = 0;
                            sector_data[base + 1] = 0;
                            sector_data[base + 2] = 0;
                            sector_data[base + 3] = 0;
                        } else {
                            let secs = sectors_per_track(t);
                            sector_data[base] = secs;
                            match secs {
                                21 => {
                                    sector_data[base + 1] = 0xFF;
                                    sector_data[base + 2] = 0xFF;
                                    sector_data[base + 3] = 0x1F;
                                }
                                19 => {
                                    sector_data[base + 1] = 0xFF;
                                    sector_data[base + 2] = 0xFF;
                                    sector_data[base + 3] = 0x07;
                                }
                                18 => {
                                    sector_data[base + 1] = 0xFF;
                                    sector_data[base + 2] = 0xFF;
                                    sector_data[base + 3] = 0x03;
                                }
                                _ => {
                                    sector_data[base + 1] = 0xFF;
                                    sector_data[base + 2] = 0xFF;
                                    sector_data[base + 3] = 0x01;
                                }
                            }
                        }
                    }
                    log::info!("BAM NAME: {:?}", self.source.name());
                    sector_data[144..160].copy_from_slice(self.source.name());
                    sector_data[160..162].copy_from_slice(b"\xA0\xA0");
                    sector_data[162..164].copy_from_slice(&self.id);
                    sector_data[164] = 0xA0;
                    sector_data[165] = 0x32;
                    sector_data[166] = 0x41;
                } else if let Some(entry) = self.shortcut_entry {
                    if sector == 1 {
                        self.encode_directory_sector(&[entry], 0, 0xFF, &mut sector_data);
                    } else {
                        // Sector > 1 is empty
                        self.encode_directory_sector(&[], 0, 0xFF, &mut sector_data);
                    }
                } else {
                    let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
                    let mut count = 0;
                    if let Some(peek) = peek_slot.take() {
                        entries[0] = peek;
                        count = 1;
                    }
                    while count < 8 {
                        if let Some(entry) = self.source.next_entry() {
                            entries[count] = entry;
                            count += 1;
                        } else {
                            break;
                        }
                    }

                    let (next_t, next_s) = if count == 0 {
                        (0, 0xFF)
                    } else if sector == 18 {
                        (0, 0xFF)
                    } else if count < 8 {
                        (0, 0xFF)
                    } else if let Some(peek) = self.source.next_entry() {
                        peek_slot = Some(peek);
                        (18, sector + 1)
                    } else {
                        (0, 0xFF)
                    };

                    self.encode_directory_sector(
                        &entries[..count],
                        next_t,
                        next_s,
                        &mut sector_data,
                    );
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

            return Ok((TrackLayout::Accurate, write_ptr, None));
        }

        // Generate Skeleton Track for other tracks
        let sectors = sectors_per_track(track);
        let speed_zone = pico1541_common::track_buffer::default_speed_zone(
            pico1541_common::track_buffer::track_to_half_track(track),
        );
        let speed_zone_len: usize = pico1541_common::track_buffer::speed_zone_capacity(speed_zone);
        let base_sector_len = 5 + 10 + 9 + 5 + 325;
        let total_base_len = sectors as usize * base_sector_len;
        let remaining_gap = speed_zone_len.saturating_sub(total_base_len);
        let gap_per_sector = remaining_gap / sectors as usize;

        let mut write_ptr = 0;
        // Lead-in
        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0x55;
                write_ptr += 1;
            }
        }

        for sector in 0..sectors {
            // Header SYNC
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
            // Header-to-Data Gap
            for _ in 0..9 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0x55;
                    write_ptr += 1;
                }
            }
            // Data SYNC
            for _ in 0..5 {
                if write_ptr < dest.len() {
                    dest[write_ptr] = 0xFF;
                    write_ptr += 1;
                }
            }
            // Empty Data Block (0x07 mark + 256 bytes 0 + 0 checksum + 0 offbyte)
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            for chunk in data_raw.chunks_exact(4) {
                let mut dst = [0u8; 5];
                gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
                if write_ptr + 5 <= dest.len() {
                    dest[write_ptr..write_ptr + 5].copy_from_slice(&dst);
                    write_ptr += 5;
                }
            }
            // Inter-sector gap
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
