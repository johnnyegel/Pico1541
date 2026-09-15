use crate::gcr;
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;

use super::{sectors_per_track, DiskImage, Error};

pub struct VirtualDiskImage<S, D: DirectorySource> {
    pub source: D,
    pub file_storage: Option<S>,
    id: [u8; 2],
    pub sector1_idx: usize,
    pub sector2_idx: usize,
    pub next_sector_to_update: u8,
    pub shortcut_entry: Option<crate::storage::DirectoryEntry>,
    pub treadmill_spinup_delay: u16,
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
            sector1_idx: 0,
            sector2_idx: 0,
            next_sector_to_update: 1,
            shortcut_entry: None,
            treadmill_spinup_delay: 0,
        }
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
        dest.copy_from_slice(&raw);
    }

    pub fn encode_directory_sector_gcr(
        &self,
        entries: &[crate::storage::DirectoryEntry],
        next_track: u8,
        next_sector: u8,
        dest_gcr: &mut [u8],
    ) {
        let mut raw = [0u8; 256];
        self.encode_directory_sector(entries, next_track, next_sector, &mut raw);

        let mut data_checksum = 0u8;
        for b in raw.iter() {
            data_checksum ^= b;
        }
        let mut data_raw = [0u8; 260];
        data_raw[0] = 0x07;
        data_raw[1..257].copy_from_slice(&raw);
        data_raw[257] = data_checksum;

        for (i, chunk) in data_raw.chunks_exact(4).enumerate() {
            crate::gcr::encode_gcr_block(
                chunk.try_into().unwrap(),
                (&mut dest_gcr[i * 5..(i + 1) * 5]).try_into().unwrap(),
            );
        }
    }

    pub async fn update_dynamic_buffer(
        &mut self,
        current_byte_idx: usize,
        gcr_data: &mut [u8],
    ) -> bool {
        if self.treadmill_spinup_delay > 0 {
            self.treadmill_spinup_delay -= 1;
            return false;
        }

        let sector = if current_byte_idx < 40 {
            0xFF
        } else {
            ((current_byte_idx - 40) / 372) as u8
        };

        let (target_sector, next_after) = if sector == 1 && self.next_sector_to_update == 2 {
            (2, 1)
        } else if sector == 2 && self.next_sector_to_update == 1 {
            (1, 2)
        } else {
            return false;
        };

        let idx = if target_sector == 1 {
            self.sector1_idx
        } else {
            self.sector2_idx
        };
        if idx == 0 {
            return false;
        }

        if let Some(storage) = &mut self.file_storage {
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

            let mut raw_sector = [0u8; 256];
            raw_sector[2..2 + bytes_read].copy_from_slice(&file_buf[..bytes_read]);

            let (next_t, next_s) = if bytes_read == 254 {
                (1, if target_sector == 1 { 2 } else { 1 })
            } else {
                (0, (bytes_read + 1) as u8) // End of file
            };
            raw_sector[0] = next_t;
            raw_sector[1] = next_s;

            let mut data_checksum = 0u8;
            for b in raw_sector.iter() {
                data_checksum ^= b;
            }
            let mut data_raw = [0u8; 260];
            data_raw[0] = 0x07;
            data_raw[1..257].copy_from_slice(&raw_sector);
            data_raw[257] = data_checksum;

            let mut dest_gcr = [0u8; 325];
            for (i, chunk) in data_raw.chunks_exact(4).enumerate() {
                crate::gcr::encode_gcr_block(
                    chunk.try_into().unwrap(),
                    (&mut dest_gcr[i * 5..(i + 1) * 5]).try_into().unwrap(),
                );
            }

            // Reverse bits for the PIO DMA (LSB first shifting)
            for byte in dest_gcr.iter_mut() {
                *byte = byte.reverse_bits();
            }

            if idx + 325 <= gcr_data.len() {
                gcr_data[idx..idx + 325].copy_from_slice(&dest_gcr);
                self.next_sector_to_update = next_after;
                return true;
            }
            return false;
        }

        let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
        let mut count = 0;

        if let Some(entry) = self.shortcut_entry {
            if target_sector == 1 {
                entries[0] = entry;
                count = 1;
            } else {
                // Sector 2 is empty/end when shortcut is present
                count = 0;
            }
        } else {
            for entry_slot in &mut entries {
                if let Some(entry) = self.source.next_entry() {
                    *entry_slot = entry;
                    count += 1;
                } else {
                    break;
                }
            }
            let is_exhausted = self
                .source
                .as_paged_mut()
                .map(|p| p.is_exhausted())
                .unwrap_or(true);
            if count == 0 && !is_exhausted {
                // Blocks not loaded yet by prefetcher, wait for next tick
                return false;
            }
        }

        log::debug!(
            "Virtual directory: Laid out {} entries in Sector {}",
            count,
            target_sector
        );

        let (next_t, next_s) = if count == 0 {
            (0, 0xFF)
        } else {
            (18, if target_sector == 1 { 2 } else { 1 })
        };

        let mut dest_gcr = [0u8; 325];
        self.encode_directory_sector_gcr(&entries[..count], next_t, next_s, &mut dest_gcr);

        // Reverse bits for the PIO DMA (LSB first shifting)
        for byte in dest_gcr.iter_mut() {
            *byte = byte.reverse_bits();
        }

        if idx + 325 <= gcr_data.len() {
            gcr_data[idx..idx + 325].copy_from_slice(&dest_gcr);
            self.next_sector_to_update = next_after;
            return true;
        }
        false
    }

    pub async fn force_update_directory_sectors(&mut self, gcr_data: &mut [u8]) {
        // To update Sector 1, we must simulate the head being in Sector 2
        self.next_sector_to_update = 1;
        self.update_dynamic_buffer(40 + 372 * 2 + 1, gcr_data).await; // Head in Sector 2, updates Sector 1

        // To update Sector 2, we must simulate the head being in Sector 1
        self.next_sector_to_update = 2;
        self.update_dynamic_buffer(40 + 372 + 1, gcr_data).await; // Head in Sector 1, updates Sector 2

        self.next_sector_to_update = 1; // When actual read starts, wait for Sector 1 to pass, then update Sector 2
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for VirtualDiskImage<S, D> {
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error> {
        log::info!("Reading VirtualDisk track {}", half_track);

        if !half_track.is_multiple_of(2) || half_track == 0 || half_track > 84 {
            return Ok((TrackLayout::Void, 0, None));
        }

        let track = half_track / 2 + 1;

        if track == 1 {
            // Track 1: Infinite File Track
            let sectors = 21;
            let speed_zone = pico1541_common::track_buffer::default_speed_zone(
                pico1541_common::track_buffer::track_to_half_track(1),
            );
            let speed_zone_len: usize =
                pico1541_common::track_buffer::speed_zone_capacity(speed_zone);
            let base_sector_len = 10 + 10 + 9 + 10 + 325; // 364
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

            if let Some(storage) = &mut self.file_storage {
                let _ = storage.seek(embedded_io_async::SeekFrom::Start(0)).await;
            }

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;
                let sector_data = [0u8; 256];

                if sector == 1 {
                    self.sector1_idx = write_ptr + 29;
                } else if sector == 2 {
                    self.sector2_idx = write_ptr + 29;
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
                    crate::gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
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
                    crate::gcr::encode_gcr_block(chunk.try_into().unwrap(), &mut dst);
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

            log::info!("VirtualDiskImage: Returning StriddenWarp for track 1");
            return Ok((TrackLayout::Warped(366), 10000, None));
        }

        // FIXME! Why is this needed?
        if half_track == 34 || half_track == 35 {
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

            for physical_sector in 0..sectors {
                let interleave = if track == 18 {
                    3
                } else if sectors == 18 {
                    11
                } else {
                    10
                };
                let sector = ((physical_sector * interleave) % sectors) as u8;
                let mut sector_data = [0u8; 256];

                if sector == 0 {
                    sector_data[0] = 18;
                    sector_data[1] = 1;
                    sector_data[2] = 0x41;
                    for b in sector_data.iter_mut().take(144).skip(4) {
                        *b = 0;
                    }
                    log::info!("BAM NAME: {:?}", self.source.name());
                    sector_data[144..160].copy_from_slice(self.source.name());
                    sector_data[160..162].copy_from_slice(b"\xA0\xA0");
                    sector_data[162..164].copy_from_slice(&self.id);
                    sector_data[164] = 0xA0;
                    sector_data[165] = 0x32;
                    sector_data[166] = 0x41;
                } else if sector == 1 || sector == 2 {
                    if sector == 1 {
                        self.sector1_idx = write_ptr + 29;
                    } else {
                        self.sector2_idx = write_ptr + 29;
                    }

                    if let Some(entry) = self.shortcut_entry {
                        if sector == 1 {
                            self.encode_directory_sector(&[entry], 0, 0xFF, &mut sector_data);
                        } else {
                            // Sector 2 is empty/end
                            self.encode_directory_sector(&[], 0, 0xFF, &mut sector_data);
                        }
                    } else {
                        let mut entries = [crate::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
                        let mut count = 0;
                        for entry_slot in &mut entries {
                            if let Some(entry) = self.source.next_entry() {
                                *entry_slot = entry;
                                count += 1;
                            } else {
                                break;
                            }
                        }

                        log::info!(
                            "Virtual directory (Initial): Laid out {} entries in Sector {}",
                            count,
                            sector
                        );

                        let (next_t, next_s) = if count == 0 {
                            (0, 0xFF)
                        } else {
                            (18, if sector == 1 { 2 } else { 1 })
                        };
                        self.encode_directory_sector(
                            &entries[..count],
                            next_t,
                            next_s,
                            &mut sector_data,
                        );
                    }
                }

                for _ in 0..10 {
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
                for _ in 0..10 {
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

            self.source.rewind();
            self.force_update_directory_sectors(dest).await;
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
            for _ in 0..10 {
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
            for _ in 0..10 {
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
