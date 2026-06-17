with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

import re

# Track 1 Loop Fix
t1_start = content.find("if track == 1 {")
t1_end = content.find("} else if track == 18 {", t1_start)

t1_new = """        if track == 1 {
            // Track 1: Infinite File Track
            let sectors = 21;
            let speed_zone_len: usize = 7692;
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

            if let Some(storage) = &mut self.file_storage {
                let _ = storage.seek(embedded_io_async::SeekFrom::Start(0)).await;
            }

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;
                let sector_data = [0u8; 256];

                if sector == 1 {
                    self.sector1_idx = write_ptr + 39;
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

            let track_bytes = write_ptr;
            return Ok((TrackLayout::Accurate, track_bytes));
"""

# Track 18 Loop Fix
t18_start = content.find("} else if track == 18 {")
t18_end = content.find("return Ok((TrackLayout::Accurate, track_bytes))", t18_start)

t18_new = """        } else if track == 18 {
            // Track 18: Virtual Directory Track
            let sectors = 19;
            let speed_zone_len: usize = 7142;
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

            self.source.reset();

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;
                let mut sector_data = [0u8; 256];

                if sector == 0 {
                    self.encode_bam_sector(&mut sector_data);
                } else {
                    if sector % 3 != 1 && sector != 1 {
                        let (next_t, next_s) = (18, if sector == 18 { 2 } else { sector + 1 });
                        sector_data[0] = next_t;
                        sector_data[1] = next_s;
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
                        
                        log::info!("Virtual directory (Initial): Laid out {} entries in Sector {}", count, sector);
                        
                        let (next_t, next_s) = if count == 0 {
                            (0, 0xFF)
                        } else {
                            (18, if sector == 1 { 4 } else if sector + 3 < 19 { sector + 3 } else { sector + 3 - 19 })
                        };
                        self.encode_directory_sector(&entries[..count], next_t, next_s, &mut sector_data);
                    }
                }

                for _ in 0..5 {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0xFF;
                        write_ptr += 1;
                    }
                }
                let checksum = (sector as u8) ^ track ^ self.id[0] ^ self.id[1];
                let header_raw = [
                    0x08, checksum, sector as u8, track, self.id[1], self.id[0], 0x0F, 0x0F,
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
            
            let track_bytes = write_ptr;
            """

content = content[:t1_start] + t1_new + t18_new + content[t18_end:]

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
