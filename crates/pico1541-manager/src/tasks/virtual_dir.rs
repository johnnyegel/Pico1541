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
    async fn open_file(&self, requested_petscii: &[u8]) -> Result<Self::FileStorage, pico1541_common::formats::Error>;
    async fn list_dir_range(
        &self,
        path: &str,
        use_internal: bool,
        start_index: usize,
        count: usize,
        callback: impl FnMut(pico1541_common::storage::DirectoryEntry),
    ) -> Result<(), pico1541_common::formats::Error>;
}

pub fn match_c64_filename(entry_name: &[u8; 16], req_name: &[u8]) -> bool {
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

    let mut e_idx = 0;
    let mut r_idx = 0;
    
    while r_idx < req_len && e_idx < entry_len {
        let r_char = req_name[r_idx];
        if r_char == 0x2A { // '*'
            return true;
        } else if r_char == 0x3F { // '?'
            // matches any char
        } else {
            if entry_name[e_idx] != r_char {
                break;
            }
        }
        e_idx += 1;
        r_idx += 1;
    }
    
    if r_idx < req_len && req_name[r_idx] == 0x2A {
        return true;
    }
    
    if r_idx == req_len && e_idx == entry_len {
        return true;
    }
    
    if r_idx == req_len {
        let extensions: &[&[u8]] = &[b".PRG", b".SEQ", b".USR", b".DEL", b".DIR"];
        for ext in extensions {
            if entry_len == r_idx + ext.len() {
                let mut match_found = true;
                for i in 0..ext.len() {
                    let ext_char = pico1541_common::storage::ascii_to_petscii(ext[i]);
                    if entry_name[r_idx + i] != ext_char {
                        match_found = false;
                        break;
                    }
                }
                if match_found {
                    return true;
                }
            }
        }
    }
    
    false
}

static mut VDISK_GCR_BUF: [u8; pico1541_common::interface::MAX_TRACK_BYTES] = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];

pub struct VirtualDiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    active_track: &'a mut ActiveTrack,
    opener: O,
    current_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
}

impl<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> VirtualDiskTask<'a, S, D, O> {
    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        active_track: &'a mut ActiveTrack,
        opener: O,
        current_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    ) -> Self {
        Self {
            disk_image,
            active_track,
            opener,
            current_dir,
        }
    }

    pub async fn run(&mut self) {
        let mut motor_on = false;
        loop {
            let timer_fut = embassy_time::Timer::after_millis(50);
            let cmd_fut = CMD_CHANNEL.receive();

            match select(timer_fut, cmd_fut).await {
                Either::First(_) => {
                    // (first_missing_block, batch_count)
                    let mut block_batch: Option<(usize, usize)> = None;
                    {
                        let mut disk_opt = self.disk_image.lock().await;
                        if let Some(disk) = disk_opt.as_mut() {
                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                // Periodic update of directory buffer only when motor is on
                                if motor_on {
                                    let current_idx = self.active_track.current_byte_idx as usize;
                                    vdir.update_dynamic_buffer(current_idx, &mut self.active_track.gcr_data);
                                }

                                // Background prefetching for paged directory source
                                if let Some(paged) = vdir.source.as_paged_mut() {
                                    // Stop prefetching once the directory end has been discovered.
                                    if !paged.is_exhausted() {
                                        let current_idx = paged.current_read_index();
                                        let current_block = current_idx / 16;
                                        let (start_block, end_block) = paged.loaded_range();

                                        // Collect consecutive missing blocks in the lookahead window.
                                        // One list_dir_range call covers them all in a single forward
                                        // scan, avoiding O(N) re-scans from entry 0.
                                        let mut batch_start = None;
                                        let mut batch_end = 0usize;
                                        for offset in 0..3 {
                                            let tb = current_block + offset;
                                            if tb < start_block || tb >= end_block {
                                                if batch_start.is_none() {
                                                    batch_start = Some(tb);
                                                    batch_end = tb + 1;
                                                } else if tb == batch_end {
                                                    batch_end += 1;
                                                }
                                            }
                                        }

                                        if let Some(bs) = batch_start {
                                            block_batch = Some((bs, batch_end - bs));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((blocks_start, blocks_count)) = block_batch {

                        // Mark active track as Void during background directory fetch to prevent Core 0 emu hang/timeouts.
                        self.active_track.layout = pico1541_common::formats::TrackLayout::Void;

                        let (path, use_internal) = {
                            let guard = self.current_dir.lock().await;
                            (guard.0.clone(), guard.1)
                        };

                        // Fetch all missing blocks in a single sequential pass.
                        // Each block holds 16 entries; use a 3×16=48 entry buffer max.
                        let fetch_start = blocks_start * 16;
                        let fetch_count = blocks_count * 16;
                        let mut all_entries = heapless::Vec::<pico1541_common::storage::DirectoryEntry, 48>::new();
                        let fetch_res = self.opener.list_dir_range(
                            path.as_str(),
                            use_internal,
                            fetch_start,
                            fetch_count,
                            |entry| {
                                let _ = all_entries.push(entry);
                            }
                        ).await;

                        if fetch_res.is_ok() {
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
                                if let Some(vdir) = disk.as_virtual_disk_mut() {
                                    if let Some(paged) = vdir.source.as_paged_mut() {
                                        // Distribute fetched entries into their correct block slots.
                                        // set_block auto-detects end-of-directory from partial blocks.
                                        for b in 0..blocks_count {
                                            let slice_start = (b * 16).min(all_entries.len());
                                            let slice_end = (slice_start + 16).min(all_entries.len());
                                            let block_slice = &all_entries[slice_start..slice_end];
                                            
                                            let mut block_entries = heapless::Vec::<pico1541_common::storage::DirectoryEntry, 16>::new();
                                            for e in block_slice {
                                                let _ = block_entries.push(*e);
                                            }
                                            paged.set_block(blocks_start + b, &block_entries);
                                            
                                            if block_slice.len() < 16 {
                                                break; // We reached the end of the directory, don't write trailing empty blocks!
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let half_track = self.active_track.half_track;
                        let gcr_buf = &raw mut VDISK_GCR_BUF;
                        let mut disk_opt = self.disk_image.lock().await;
                        if let Some(disk) = disk_opt.as_mut() {
                            match disk.read_track(half_track, unsafe { &mut *gcr_buf }).await {
                                Ok(layout) => {
                                    self.active_track.gcr_data.copy_from_slice(unsafe { &*gcr_buf });
                                    self.active_track.layout = layout;
                                }
                                Err(e) => {
                                    error!("Failed to restore active track: {:?}", e);
                                    self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
                                }
                            }
                        } else {
                            self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
                        }
                    }
                }
                Either::Second(cmd) => {
                    match cmd {
                        InternalCommand::HeadStepped(half_track) => {
                            info!("Manager loading track {}", half_track);
                            let gcr_buf = &raw mut VDISK_GCR_BUF;
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
                                match disk.read_track(half_track, unsafe { &mut *gcr_buf }).await {
                                    Ok(layout) => {
                                        self.active_track.gcr_data.copy_from_slice(unsafe { &*gcr_buf });
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
                        InternalCommand::MotorState(active) => {
                            motor_on = active;
                        }
                        InternalCommand::ResetDirectory => {
                            info!("Motor ON detected. Resetting virtual directory to start.");
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
                                if let Some(vdir) = disk.as_virtual_disk_mut() {
                                    vdir.source.rewind();
                                    if let Some(paged) = vdir.source.as_paged_mut() {
                                        paged.invalidate();
                                    }
                                    vdir.force_update_directory_sectors(&mut self.active_track.gcr_data);
                                }
                            }
                        }
                        InternalCommand::ImageMounted => {
                            info!("Image mounted. Re-reading current track.");
                            let half_track = self.active_track.half_track;
                            let gcr_buf = &raw mut VDISK_GCR_BUF;
                            let mut disk_opt = self.disk_image.lock().await;
                            if let Some(disk) = disk_opt.as_mut() {
                                match disk.read_track(half_track, unsafe { &mut *gcr_buf }).await {
                                    Ok(layout) => {
                                        self.active_track.gcr_data.copy_from_slice(unsafe { &*gcr_buf });
                                        self.active_track.layout = layout;
                                    }
                                    Err(e) => {
                                        error!("Failed to read track on mount: {:?}", e);
                                        self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
                                    }
                                }
                            } else {
                                self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
                            }
                        }
                        InternalCommand::LoadFile(filename) => {
                            let is_dir = filename[0] == b'$';
                            
                            if is_dir {
                                info!("Directory requested. Rewinding.");
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        vdir.set_shortcut_entry(None);
                                        vdir.file_storage = None;
                                        vdir.source.rewind();
                                        if let Some(paged) = vdir.source.as_paged_mut() {
                                            paged.invalidate();
                                        }
                                    }
                                }
                                drop(disk_opt);
                                
                                // Fetch first block synchronously to prevent empty directory
                                let (dir_path, use_internal) = {
                                    let guard = self.current_dir.lock().await;
                                    (guard.0.clone(), guard.1)
                                };
                                
                                let mut all_entries = heapless::Vec::<pico1541_common::storage::DirectoryEntry, 48>::new();
                                let _ = self.opener.list_dir_range(
                                    dir_path.as_str(),
                                    use_internal,
                                    0,
                                    16,
                                    |entry| {
                                        let _ = all_entries.push(entry);
                                    }
                                ).await;
                                
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        if let Some(paged) = vdir.source.as_paged_mut() {
                                            paged.set_block(0, &all_entries);
                                            if all_entries.len() < 16 {
                                                paged.set_total_count(all_entries.len());
                                            }
                                        }
                                        vdir.force_update_directory_sectors(&mut self.active_track.gcr_data);
                                    }
                                }
                            } else {
                                info!("Opening file in Virtual Disk...");
                                match self.opener.open_file(&filename).await {
                                    Ok(file_storage) => {
                                        info!("File found! Setting shortcut entry.");
                                        let mut entry_name = [0xA0; 16];
                                        let copy_len = filename.len().min(16);
                                        entry_name[..copy_len].copy_from_slice(&filename[..copy_len]);
                                        
                                        let shortcut = pico1541_common::storage::DirectoryEntry::new(&entry_name, 0x82, 1, 0);
                                        let mut disk_opt = self.disk_image.lock().await;
                                        if let Some(disk) = disk_opt.as_mut() {
                                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                                vdir.set_shortcut_entry(Some(shortcut));
                                                vdir.file_storage = Some(file_storage);
                                                vdir.force_update_directory_sectors(&mut self.active_track.gcr_data);
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        warn!("File not found in Virtual Disk.");
                                        let mut disk_opt = self.disk_image.lock().await;
                                        if let Some(disk) = disk_opt.as_mut() {
                                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                                vdir.set_shortcut_entry(None);
                                                vdir.file_storage = None;
                                                vdir.force_update_directory_sectors(&mut self.active_track.gcr_data);
                                            }
                                        }
                                    }
                                }
                            }

                            let needs_regeneration = self.active_track.layout == pico1541_common::formats::TrackLayout::Void;
                            if needs_regeneration {
                                let half_track = self.active_track.half_track;
                                let gcr_buf = &raw mut VDISK_GCR_BUF;
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    match disk.read_track(half_track, unsafe { &mut *gcr_buf }).await {
                                        Ok(layout) => {
                                            self.active_track.gcr_data.copy_from_slice(unsafe { &*gcr_buf });
                                            self.active_track.layout = layout;
                                            self.active_track.half_track = half_track;
                                        }
                                        Err(_) => {
                                            self.active_track.layout = pico1541_common::formats::TrackLayout::Void;
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
}
