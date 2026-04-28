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
                vdir.update_dynamic_buffer(current_idx, &mut self.active_track.gcr_data);
            }

            embassy_time::Timer::after_millis(10).await;
        }
    }
}
