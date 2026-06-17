with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

import re
# We need to rewrite the sector layout loop in D64Image::read_track

new_loop = """
        let speed_zone_len = if track <= 17 { 7692 } else if track <= 24 { 7142 } else if track <= 30 { 6666 } else { 6250 };
        let base_sector_len = 5 + 10 + 325; // 5 sync + 10 header + 325 data
        let total_base_len = sectors * base_sector_len;
        let remaining_gap = speed_zone_len - total_base_len;
        let gap_per_sector = remaining_gap / sectors;

        for _ in 0..40 {
            if write_ptr < dest.len() {
                dest[write_ptr] = 0x55;
                write_ptr += 1;
            }
        }
        for logical_sector in 0..sectors {
            let mut sector_data = [0u8; 256];
            let sector = logical_sector;
            
            self.read_sector_data(offset as u64 + (sector as u64 * 256), &mut sector_data).await?;
                
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
            for _ in 0..10 {
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
"""

start_idx = content.find("let mut write_ptr = 0;")
end_idx = content.find("let track_bytes = write_ptr;", start_idx)

new_content = content[:start_idx + len("let mut write_ptr = 0;")] + new_loop + content[end_idx:]

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(new_content)
