use embedded_io_async::{Read, Seek};
use pico1541_common::formats::{AnyDiskImage, DiskImage};
use pico1541_common::interface::ActiveTrack;
use pico1541_common::storage::DirectorySource;
use crate::{CMD_CHANNEL, InternalCommand};
use embassy_futures::select::{select, Either};
use log::*;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

pub trait FileOpener {
    type FileStorage: embedded_io_async::Read + embedded_io_async::Seek + 'static;
    async fn open_file(&self, name: &str) -> Result<Self::FileStorage, pico1541_common::formats::Error>;
}

fn match_c64_filename(entry_name: &[u8; 16], req_name: &[u8]) -> bool {
    let mut entry_len = 16;
    while entry_len > 0 && entry_name[entry_len - 1] == 0xA0 {
        entry_len -= 1;
    }
    
    let mut req_len = req_name.len();
    while req_len > 0 && req_name[req_len - 1] == 0xA0 {
        req_len -= 1;
    }
    
    if req_len == 0 || entry_len == 0 {
        return false;
    }
    
    if entry_len == req_len {
        let mut exact = true;
        for i in 0..entry_len {
            if entry_name[i].to_ascii_uppercase() != req_name[i].to_ascii_uppercase() {
                exact = false;
                break;
            }
        }
        if exact {
            return true;
        }
    }
    
    let extensions: &[&[u8]] = &[b".PRG", b".SEQ", b".USR", b".DEL"];
    for ext in extensions {
        if entry_len == req_len + ext.len() {
            let mut match_found = true;
            for i in 0..req_len {
                if entry_name[i].to_ascii_uppercase() != req_name[i].to_ascii_uppercase() {
                    match_found = false;
                    break;
                }
            }
            if match_found {
                for i in 0..ext.len() {
                    if entry_name[req_len + i].to_ascii_uppercase() != ext[i] {
                        match_found = false;
                        break;
                    }
                }
            }
            if match_found {
                return true;
            }
        }
    }
    
    false
}

pub struct VirtualDiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    active_track: &'a mut ActiveTrack,
    opener: O,
}

impl<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> VirtualDiskTask<'a, S, D, O> {
    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        active_track: &'a mut ActiveTrack,
        opener: O,
    ) -> Self {
        Self {
            disk_image,
            active_track,
            opener,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let timer_fut = embassy_time::Timer::after_millis(50);
            let cmd_fut = CMD_CHANNEL.receive();

            match select(timer_fut, cmd_fut).await {
                Either::First(_) => {
                    // Periodic update of directory buffer
                    let mut disk_opt = self.disk_image.lock().await;
                    if let Some(disk) = disk_opt.as_mut() {
                        if let Some(vdir) = disk.as_virtual_disk_mut() {
                            let current_idx = self.active_track.current_byte_idx as usize;
                            vdir.update_dynamic_buffer(current_idx, &mut self.active_track.gcr_data);
                        }
                    }
                }
                Either::Second(cmd) => {
                    match cmd {
                        InternalCommand::HeadStepped(half_track) => {
                            info!("Manager loading track {}", half_track);
                            let mut gcr_buf = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
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
                            } else {
                                self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
                                self.active_track.half_track = half_track;
                            }
                        }
                        InternalCommand::LoadFile(filename) => {
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
                                if let Some(vdir) = disk.as_virtual_disk_mut() {
                                    info!("Searching for file in Virtual Disk...");
                                    vdir.source.rewind();
                                    let mut found_entry = None;
                                    while let Some(entry) = vdir.source.next_entry() {
                                        if match_c64_filename(&entry.filename, &filename) {
                                            found_entry = Some(entry);
                                            break;
                                        }
                                    }

                                    if let Some(entry) = found_entry {
                                        info!("File found! Setting shortcut entry.");
                                        let mut shortcut = entry;
                                        shortcut.first_track = 1;
                                        shortcut.first_sector = 0;
                                        vdir.set_shortcut_entry(Some(shortcut));

                                        let mut ascii_name = [0u8; 16];
                                        let len = pico1541_common::disk_info::petscii_to_ascii(&entry.filename, &mut ascii_name);
                                        let name_str = core::str::from_utf8(&ascii_name[..len]).unwrap_or("");
                                        match self.opener.open_file(name_str).await {
                                            Ok(file_storage) => {
                                                vdir.file_storage = Some(file_storage);
                                            }
                                            Err(e) => {
                                                error!("Failed to open file data for virtual disk: {:?}", e);
                                                vdir.file_storage = None;
                                            }
                                        }
                                    } else {
                                        warn!("File not found in Virtual Disk.");
                                        vdir.set_shortcut_entry(None);
                                        vdir.file_storage = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
