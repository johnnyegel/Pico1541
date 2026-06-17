with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

import re

# Track 1
content = re.sub(
    r"let mut write_ptr = 0;\s*for _ in 0\.\.40 \{\s*if write_ptr < dest\.len\(\) \{\s*dest\[write_ptr\] = 0x55;\s*write_ptr \+= 1;\s*\}\s*\}\s*if let Some\(storage\) = &mut self\.file_storage \{\s*let _ = storage\.seek\(embedded_io_async::SeekFrom::Start\(0\)\)\.await;\s*\}\s*for physical_sector in 0\.\.sectors \{\s*let interleave = if track == 18 \{ 3 \} else if sectors == 18 \{ 11 \} else \{ 10 \};\s*let sector = \(\(physical_sector \* interleave\) % sectors\) as u8;",
    """let speed_zone_len: usize = 7692;
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
                let sector = physical_sector as u8;""",
    content
)

content = content.replace("for _ in 0..16 {\n                    if write_ptr < dest.len() {\n                        dest[write_ptr] = 0x55;\n                        write_ptr += 1;\n                    }\n                }\n            }\n\n            log::info!(\"VirtualDiskImage: Returning StriddenWarp for track 1\");\n            return Ok((TrackLayout::Warped(372), 10000));",
"""for _ in 0..gap_per_sector {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0x55;
                        write_ptr += 1;
                    }
                }
            }

            log::info!("VirtualDiskImage: Returning StriddenWarp for track 1");
            return Ok((TrackLayout::Warped(366), 10000));""")

# Track 18
content = re.sub(
    r"let mut write_ptr = 0;\s*for _ in 0\.\.40 \{\s*if write_ptr < dest\.len\(\) \{\s*dest\[write_ptr\] = 0x55;\s*write_ptr \+= 1;\s*\}\s*\}\s*self\.source\.reset\(\);\s*for physical_sector in 0\.\.sectors \{\s*let interleave = if track == 18 \{ 3 \} else if sectors == 18 \{ 11 \} else \{ 10 \};\s*let sector = \(\(physical_sector \* interleave\) % sectors\) as u8;",
    """let speed_zone_len: usize = 7142;
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

            self.source.reset();

            for physical_sector in 0..sectors {
                let sector = physical_sector as u8;""",
    content
)

content = content.replace("for _ in 0..16 {\n                    if write_ptr < dest.len() {\n                        dest[write_ptr] = 0x55;\n                        write_ptr += 1;\n                    }\n                }\n            }\n            \n            let track_bytes = write_ptr;\n            return Ok((TrackLayout::Accurate, track_bytes));",
"""for _ in 0..gap_per_sector {
                    if write_ptr < dest.len() {
                        dest[write_ptr] = 0x55;
                        write_ptr += 1;
                    }
                }
            }
            
            let track_bytes = write_ptr;
            return Ok((TrackLayout::Accurate, track_bytes));""")

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
