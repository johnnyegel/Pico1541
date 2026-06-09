use embedded_io_async::{Read, Seek};
use pico1541_common::formats::{AnyDiskImage, DiskImage};
use pico1541_common::interface::ActiveTrack;
use pico1541_common::storage::DirectorySource;
use crate::{CMD_CHANNEL, TRACK_CHANNEL, InternalCommand};
use embassy_futures::select::{select, select3, Either, Either3};
#[cfg(feature = "defmt")]
use defmt::{info, warn, error};
#[cfg(not(feature = "defmt"))]
use log::{info, warn, error};

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


pub struct VirtualDiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    track_buffers: &'static pico1541_common::interface::DoubleTrackBuffer,
    opener: O,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
}

impl<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> VirtualDiskTask<'a, S, D, O> {
    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        track_buffers: &'static pico1541_common::interface::DoubleTrackBuffer,
        opener: O,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    ) -> Self {
        Self {
            disk_image,
            track_buffers,
            opener,
            mounted_dir,
        }
    }

    #[inline(always)]
    fn active_track_mut(&self) -> &mut ActiveTrack {
        unsafe { &mut *self.track_buffers.get_active_ptr() }
    }

    #[inline(always)]
    fn inactive_track_mut(&self) -> &mut ActiveTrack {
        unsafe { &mut *self.track_buffers.get_inactive_ptr() }
    }

    async fn handle_track_event(&mut self, half_track: u8, drive_track: u8, drive_sector: u8) {
        // Drain any immediately pending track events from the dedicated channel
        // to skip intermediate tracks and avoid excessive SD card reads.
        let mut final_half_track = half_track;
        let mut final_drive_track = drive_track;
        let mut final_drive_sector = drive_sector;
        while let Ok((ht, dt, ds)) = TRACK_CHANNEL.try_receive() {
            final_half_track = ht;
            final_drive_track = dt;
            final_drive_sector = ds;
        }

        info!("Manager loading track {} (ram: T={}, S={})", final_half_track, final_drive_track, final_drive_sector);
        
        // Prevent DOS from reading the old track data (and wrong headers) by stalling it safely.
        let old_layout = self.active_track_mut().layout.clone();
        self.active_track_mut().layout = pico1541_common::formats::TrackLayout::SyncWait;

        let mut disk_opt = self.disk_image.lock().await;
        if let Some(disk) = disk_opt.as_mut() {
            let inactive = self.inactive_track_mut();
            match disk.read_track(final_half_track, &mut inactive.gcr_data).await {
                Ok(layout) => {
                    inactive.layout = layout;
                    inactive.half_track = final_half_track;
                    inactive.is_dirty = false;
                    self.track_buffers.swap();
                }
                Err(e) => {
                    error!("Failed to read track: {:?}", &e);
                    self.active_track_mut().layout = old_layout;
                }
            }
        } else {
            self.active_track_mut().layout = pico1541_common::formats::TrackLayout::Void;
            self.active_track_mut().half_track = final_half_track;
        }
    }

    pub async fn run(&mut self) {
        let mut motor_on = false;
        loop {
            // Check if there is a pending track event IMMEDIATELY.
            // If so, handle it first to prevent select3 from randomly choosing the timer/cmd.
            if let Ok((half_track, drive_track, drive_sector)) = TRACK_CHANNEL.try_receive() {
                self.handle_track_event(half_track, drive_track, drive_sector).await;
                continue;
            }

            let track_fut = TRACK_CHANNEL.receive();
            let timer_fut = embassy_time::Timer::after_millis(20);
            let cmd_fut = CMD_CHANNEL.receive();

            match select3(track_fut, timer_fut, cmd_fut).await {
                Either3::First((half_track, drive_track, drive_sector)) => {
                    self.handle_track_event(half_track, drive_track, drive_sector).await;
                }
                Either3::Second(_) => {
                    // (first_missing_block, batch_count)
                    let mut block_batch: Option<(usize, usize)> = None;
                    {
                        let mut disk_opt = self.disk_image.lock().await;
                        if let Some(disk) = disk_opt.as_mut() {
                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                // Periodic update of directory buffer only when motor is on
                                if motor_on {
                                    let current_idx = pico1541_common::interface::CURRENT_TRACK_BYTE_IDX.load(core::sync::atomic::Ordering::Relaxed) as usize;
                                    
                                    if vdir.file_storage.is_some() {
                                        // Track 1: File streaming (StriddenWarp)
                                        vdir.update_dynamic_buffer(current_idx, &mut self.active_track_mut().gcr_data).await;
                                    } else if !vdir.source.as_paged_mut().map(|p| p.is_exhausted()).unwrap_or(true) {
                                        // Track 18: Directory Listing (Accurate(7142))
                                        let s1 = vdir.sector1_idx;
                                        let s2 = vdir.sector2_idx;
                                        
                                        if s1 > 0 && s2 > 0 {
                                            let s2_end = s2 + 325;
                                            let s1_start = s1.saturating_sub(40);
                                            
                                            // Dead zone between Sector 2 and Sector 1
                                            if current_idx > s2_end && current_idx < s1_start {
                                                if vdir.next_sector_to_update == 2 {
                                                    // Head has passed Sector 2. Swap to the next batch!
                                                    self.inactive_track_mut().gcr_data.copy_from_slice(&self.active_track_mut().gcr_data);
                                                    vdir.force_update_directory_sectors(&mut self.inactive_track_mut().gcr_data).await;
                                                    self.track_buffers.swap();
                                                    
                                                    // Prevent further swaps until we pass Sector 2 again
                                                    vdir.next_sector_to_update = 1;
                                                }
                                            } else if current_idx < s2 {
                                                // Reset the trigger flag when head loops around before Sector 2
                                                vdir.next_sector_to_update = 2;
                                            }
                                        }
                                    }
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

                        // Mark active track as SyncWait during background directory fetch to prevent Core 0 emu hang/timeouts.
                        let old_layout = self.active_track_mut().layout.clone();
                        self.active_track_mut().layout = pico1541_common::formats::TrackLayout::SyncWait;

                        let (path, use_internal) = {
                            let guard = self.mounted_dir.lock().await;
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

                                        // We fetched new blocks. If the active track is 18, we must regenerate the GCR data!
                                        let active_ht = self.active_track_mut().half_track;
                                        if active_ht == 34 || active_ht == 35 {
                                            vdir.next_sector_to_update = 1;
                                            vdir.force_update_directory_sectors(&mut self.active_track_mut().gcr_data).await;
                                            vdir.treadmill_spinup_delay = 10;
                                            self.active_track_mut().is_dirty = false;
                                        }
                                    }
                                }
                            }
                        }

                        self.active_track_mut().layout = old_layout;
                    }
                }
                Either3::Third(cmd) => {
                    match cmd {
                        InternalCommand::MotorState(active) => {
                            if active && !motor_on {
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        vdir.source.rewind();
                                        vdir.next_sector_to_update = 1;
                                        if self.active_track_mut().is_dirty {
                                            let gcr_buf = &mut self.active_track_mut().gcr_data;
                                            vdir.force_update_directory_sectors(gcr_buf).await;
                                            vdir.treadmill_spinup_delay = 10;
                                            self.active_track_mut().is_dirty = false;
                                        }
                                    }
                                }
                            }
                            motor_on = active;
                        }

                        InternalCommand::ImageMounted => {
                            info!("Image mounted. Simulating disk insertion.");
                            
                            // 1. Unmount sequence (in case a disk was already there)
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().layout, pico1541_common::formats::TrackLayout::Void); }
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, true); } // LOW (sliding out)
                            embassy_time::Timer::after_millis(50).await;
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, false); } // HIGH (no disk)
                            
                            let mut disk_opt = self.disk_image.lock().await;
                            if !disk_opt.is_none() {
                                embassy_time::Timer::after_millis(400).await;
                                unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, true); } // LOW (sliding in)
                                embassy_time::Timer::after_millis(50).await;
                                unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, true); } // LOW (configured: always write protected)
                                
                                // Set force disk change flag
                                unsafe { core::ptr::write_volatile(&mut self.active_track_mut().force_disk_change, true); }

                                let half_track = self.active_track_mut().half_track;
                                
                                // Initialize Virtual Directory if applicable before reading track
                                let mut is_virtual_disk = false;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        is_virtual_disk = true;
                                        vdir.set_shortcut_entry(None);
                                        vdir.file_storage = None;
                                        vdir.source.rewind();
                                        if let Some(paged) = vdir.source.as_paged_mut() {
                                            paged.invalidate();
                                        }
                                    }
                                }
                                drop(disk_opt);
                                
                                if is_virtual_disk {
                                    let (dir_path, use_internal) = {
                                        let guard = self.mounted_dir.lock().await;
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
                                    
                                    info!("ImageMounted: Fetched {} entries from {}", all_entries.len(), dir_path.as_str());

                                    let mut disk_opt = self.disk_image.lock().await;
                                    if let Some(disk) = disk_opt.as_mut() {
                                        if let Some(vdir) = disk.as_virtual_disk_mut() {
                                            if let Some(paged) = vdir.source.as_paged_mut() {
                                                paged.set_block(0, &all_entries);
                                                if all_entries.len() < 16 {
                                                    paged.set_total_count(all_entries.len());
                                                }
                                            }
                                        }
                                    }
                                }

                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    let inactive = self.inactive_track_mut();
                                    match disk.read_track(half_track, &mut inactive.gcr_data).await {
                                        Ok(layout) => {
                                            unsafe { core::ptr::write_volatile(&mut inactive.layout, layout); }
                                            inactive.half_track = half_track;
                                            inactive.is_dirty = false;
                                            self.track_buffers.swap();
                                        }
                                        Err(e) => {
                                            error!("Failed to read track on mount: {:?}", &e);
                                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().layout, pico1541_common::formats::TrackLayout::Void); }
                                        }
                                    }
                                }
                            }
                        }
                        InternalCommand::ImageUnmounted => {
                            info!("Image unmounted. Simulating disk removal.");
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().layout, pico1541_common::formats::TrackLayout::Void); }
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, true); } // LOW (sliding out)
                            embassy_time::Timer::after_millis(50).await;
                            unsafe { core::ptr::write_volatile(&mut self.active_track_mut().is_write_protected, false); } // HIGH (no disk)
                            
                            // Wait a bit for the C64 DOS to process the WP change before logging the final state
                            embassy_time::Timer::after_millis(50).await;
                            
                            info!("Image unmounted. Simulating disk removal completed.");
                        }
                        InternalCommand::CloseChannel(sa) => {
                            if sa == 0 {
                                info!("Channel 0 closed. Rewinding virtual directory.");
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        vdir.source.rewind();
                                        vdir.next_sector_to_update = 1;
                                        vdir.force_update_directory_sectors(&mut self.active_track_mut().gcr_data).await;
                                        vdir.treadmill_spinup_delay = 10;
                                    }
                                }
                            }
                        }
                        InternalCommand::LoadFile(filename) => {
                            let is_dir = filename[0] == b'$';
                            
                            if is_dir {
                                info!("Directory requested. Rewinding.");
                                
                                // Reset shortcut logic
                                let mut is_virtual_disk = false;
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                                        is_virtual_disk = true;
                                        vdir.set_shortcut_entry(None);
                                        vdir.file_storage = None;
                                        vdir.source.rewind();
                                        if let Some(paged) = vdir.source.as_paged_mut() {
                                            paged.invalidate();
                                        }
                                    }
                                }
                                drop(disk_opt);
                                
                                if is_virtual_disk {
                                    // Temporarily set layout to SyncWait so the 1541 DOS waits for SYNC
                                    // while we fetch the directory, preventing race conditions and timeouts.
                                    let old_layout = self.active_track_mut().layout.clone();
                                    self.active_track_mut().layout = pico1541_common::formats::TrackLayout::SyncWait;
                                    
                                    // Fetch first block synchronously to prevent empty directory
                                    let (dir_path, use_internal) = {
                                        let guard = self.mounted_dir.lock().await;
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
                                            vdir.next_sector_to_update = 1;
                                            vdir.force_update_directory_sectors(&mut self.active_track_mut().gcr_data).await;
                                            vdir.treadmill_spinup_delay = 10;
                                        }
                                    }
                                    self.active_track_mut().layout = old_layout;
                                }
                            } else {
                                info!("Opening file in Virtual Disk...");
                                // Set layout to Void while opening, to prevent reading while state is unstable
                                let old_layout = self.active_track_mut().layout.clone();
                                self.active_track_mut().layout = pico1541_common::formats::TrackLayout::SyncWait;

                                match self.opener.open_file(&filename).await {
                                    Ok(file_storage) => {
                                        info!("File found! Setting shortcut entry.");
                                        let mut entry_name = [0xA0; 16];
                                        let copy_len = filename.len().min(16);
                                        entry_name[..copy_len].copy_from_slice(&filename[..copy_len]);
                                        
                                        let shortcut = pico1541_common::storage::DirectoryEntry::new(&entry_name, 0x82, 1, 1);
                                        let mut disk_opt = self.disk_image.lock().await;
                                        if let Some(disk) = disk_opt.as_mut() {
                                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                                vdir.set_shortcut_entry(Some(shortcut));
                                                vdir.file_storage = Some(file_storage);
                                                vdir.force_update_directory_sectors(&mut self.active_track_mut().gcr_data).await;
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
                                                vdir.force_update_directory_sectors(&mut self.active_track_mut().gcr_data).await;
                                            }
                                        }
                                    }
                                }
                                self.active_track_mut().layout = old_layout;
                            }

                            let needs_regeneration = self.active_track_mut().layout == pico1541_common::formats::TrackLayout::Void;
                            if needs_regeneration {
                                let half_track = self.active_track_mut().half_track;
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    let inactive = self.inactive_track_mut();
                                    match disk.read_track(half_track, &mut inactive.gcr_data).await {
                                        Ok(layout) => {
                                            unsafe { core::ptr::write_volatile(&mut inactive.layout, layout); }
                                            inactive.half_track = half_track;
                                            inactive.is_dirty = false;
                                            self.track_buffers.swap();
                                        }
                                        Err(_) => {
                                            self.active_track_mut().layout = pico1541_common::formats::TrackLayout::Void;
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
