use pico1541_common::formats::{AnyDiskImage, VirtualDirImage};
use pico1541_common::storage::DirectorySource;
use pico1541_common::interface::ActiveTrack;
use embedded_io_async::{Read, Seek};
use crate::gcr;

pub struct VirtualDirTask<'a, S: Read + Seek, D: DirectorySource> {
    disk_image: &'a mut AnyDiskImage<S, D>,
    active_track: &'a mut ActiveTrack,
    last_sector: u8,
}

impl<'a, S: Read + Seek, D: DirectorySource> VirtualDirTask<'a, S, D> {
    pub fn new(disk_image: &'a mut AnyDiskImage<S, D>, active_track: &'a mut ActiveTrack) -> Self {
        Self {
            disk_image,
            active_track,
            last_sector: 0,
        }
    }

    pub async fn run(&mut self) {
        loop {
            // Only act if we are on Track 18
            // We need a way to know the current track, but for now we assume Track 18
            // or we could check some state.
            
            if let Some(vdir) = self.disk_image.as_virtual_dir_mut() {
                let current_idx = self.active_track.current_byte_idx as usize;
                
                // Determine which sector we are in. 
                // Using 372 bytes per sector and 40 bytes lead-in.
                let sector = if current_idx < 40 {
                    0xFF // Lead-in
                } else {
                    ((current_idx - 40) / 372) as u8
                };

                if sector != self.last_sector {
                    // Head moved to a new sector
                    if sector == 1 {
                        // Head is in Sector 1, so we can safely update Sector 2
                        self.update_sector(vdir, 2).await;
                    } else if sector == 2 {
                        // Head is in Sector 2, so we can safely update Sector 1
                        self.update_sector(vdir, 1).await;
                    }
                    self.last_sector = sector;
                }
            }

            embassy_time::Timer::after_millis(10).await;
        }
    }

    async fn update_sector(&mut self, vdir: &mut VirtualDirImage<D>, sector_to_update: u8) {
        let mut entries = [pico1541_common::storage::DirectoryEntry::new(b"", 0, 0, 0); 8];
        let mut count = 0;
        for i in 0..8 {
            if let Some(entry) = vdir.source.next_entry() {
                entries[i] = entry;
                count += 1;
            } else {
                break;
            }
        }

        if count == 0 {
            // No more entries, link to nothing (or back to S1/S2 depending on logic)
            // For infinite loop, we might want to rewind or just stop.
            // If the user wants to see everything, we should probably stop linking.
            let mut sector_data = [0u8; 256];
            vdir.encode_directory_sector(&[], 0, 0xFF, &mut sector_data);
            self.write_gcr_sector(vdir, sector_to_update, &sector_data);
            return;
        }

        let mut sector_data = [0u8; 256];
        // Link to the OTHER sector
        let next_s = if sector_to_update == 1 { 2 } else { 1 };
        vdir.encode_directory_sector(&entries[..count], 18, next_s, &mut sector_data);
        
        self.write_gcr_sector(vdir, sector_to_update, &sector_data);
    }

    fn write_gcr_sector(&mut self, vdir: &VirtualDirImage<D>, sector: u8, data: &[u8; 256]) {
        let idx = if sector == 1 { vdir.sector1_idx } else { vdir.sector2_idx };
        if idx == 0 { return; }

        let mut data_checksum = 0u8;
        for b in data.iter() { data_checksum ^= b; }
        let mut data_raw = [0u8; 260];
        data_raw[0] = 0x07;
        data_raw[1..257].copy_from_slice(data);
        data_raw[257] = data_checksum;

        for i in 0..65 {
            let mut src = [0u8; 4];
            src.copy_from_slice(&data_raw[i*4..(i+1)*4]);
            let mut dst = [0u8; 5];
            gcr::encode_gcr_block(&src, &mut dst);
            let write_pos = idx + i * 5;
            if write_pos + 5 <= self.active_track.gcr_data.len() {
                self.active_track.gcr_data[write_pos..write_pos+5].copy_from_slice(&dst);
            }
        }
    }
}
