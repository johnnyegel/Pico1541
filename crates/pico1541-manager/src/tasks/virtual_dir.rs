use embedded_io_async::{Read, Seek};
use pico1541_common::formats::{AnyDiskImage, DiskImage};
use pico1541_common::interface::ActiveTrack;
use pico1541_common::storage::DirectorySource;
use crate::{CMD_CHANNEL, InternalCommand};
use embassy_futures::select::{select, Either};
use log::*;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

pub struct VirtualDiskTask<'a, S: Read + Seek, D: DirectorySource> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, AnyDiskImage<S, D>>,
    active_track: &'a mut ActiveTrack,
}

impl<'a, S: Read + Seek, D: DirectorySource> VirtualDiskTask<'a, S, D> {
    pub fn new(disk_image: &'a Mutex<CriticalSectionRawMutex, AnyDiskImage<S, D>>, active_track: &'a mut ActiveTrack) -> Self {
        Self {
            disk_image,
            active_track,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let timer_fut = embassy_time::Timer::after_millis(50);
            let cmd_fut = CMD_CHANNEL.receive();

            match select(timer_fut, cmd_fut).await {
                Either::First(_) => {
                    // Periodic update of directory buffer
                    let mut disk = self.disk_image.lock().await;
                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                        let current_idx = self.active_track.current_byte_idx as usize;
                        vdir.update_dynamic_buffer(current_idx, &mut self.active_track.gcr_data);
                    }
                }
                Either::Second(cmd) => {
                    match cmd {
                        InternalCommand::HeadStepped(half_track) => {
                            info!("Manager loading track {}", half_track);
                            let mut gcr_buf = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
                            let mut disk = self.disk_image.lock().await;
                            match disk.read_track(half_track, &mut gcr_buf).await {
                                Ok(layout) => {
                                    self.active_track.gcr_data.copy_from_slice(&gcr_buf);
                                    self.active_track.layout = layout;
                                    self.active_track.half_track = half_track;
                                }
                                Err(e) => {
                                    error!("Failed to read track: {:?}", e);
                                }
                            }
                        }
                        InternalCommand::LoadFile(filename) => {
                            let mut disk = self.disk_image.lock().await;
                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                info!("Searching for file in Virtual Disk...");
                                if let Some(entry) = vdir.source.find_entry(&filename) {
                                    info!("File found! Setting shortcut entry.");
                                    let mut shortcut = entry;
                                    shortcut.first_track = 1;
                                    shortcut.first_sector = 0;
                                    vdir.set_shortcut_entry(Some(shortcut));
                                } else {
                                    warn!("File not found in Virtual Disk.");
                                    vdir.set_shortcut_entry(None);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
