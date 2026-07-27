use embedded_io_async::{Read, Seek};
use pico1541_common::formats::{AnyDiskImage, DiskImage};
use pico1541_common::track_buffer::{ManagerHandle, TrackLayout};
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
        } else if entry_name[e_idx] != r_char {
            break;
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


pub struct DiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {
    disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
    track_handle: ManagerHandle,
    opener: O,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
}

impl<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> DiskTask<'a, S, D, O> {
    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        track_handle: ManagerHandle,
        opener: O,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    ) -> Self {
        Self {
            disk_image,
            track_handle,
            opener,
            mounted_dir,
        }
    }

    async fn handle_track_event(&mut self, half_track: u8) {
        let mut target_ht = half_track;

        loop {
            // Drain queue
            while let Ok(ht) = TRACK_CHANNEL.try_receive() {
                target_ht = ht;
            }
            
            info!("Manager loading half-track: {}", target_ht);

            // Prevent DOS from reading the old track data by stalling it safely.
            let old_layout = self.track_handle.get_active_layout();
            self.track_handle.set_active_layout(TrackLayout::Void);

            let mut disk_opt = self.disk_image.lock().await;
            if let Some(disk) = disk_opt.as_mut() {
                let data = self.track_handle.start_track_load();
                
                let start_time = embassy_time::Instant::now();
                let track_change_fut = async {
                    loop {
                        let current_ht = crate::SHARED_HALF_TRACK.load(portable_atomic::Ordering::Relaxed);
                        if current_ht != target_ht {
                            return current_ht;
                        }
                        embassy_time::Timer::after_millis(1).await;
                    }
                };

                // Read track but CANCEL if the head steps again!
                match select(disk.read_track(target_ht, data), track_change_fut).await {
                    Either::First(Ok(layout)) => {
                        self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(target_ht, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(target_ht))), layout.0, layout.1);
                        let elapsed = start_time.elapsed().as_millis();
                        info!("Track load took {} ms", elapsed);
                        if layout.1 >= 16 {
                            info!("Track {} loaded!", target_ht);
                        }
                        break; // Success, done loading the track!
                    }
                    Either::First(Err(e)) => {
                        error!("Failed to read track: {:?}", &e);
                        self.track_handle.set_active_layout(old_layout);
                        break;
                    }
                    Either::Second(new_ht) => {
                        // The head stepped WHILE we were reading from the SD card!
                        // The read is aborted. We loop around to start reading the NEW track!
                        target_ht = new_ht;
                        continue;
                    }
                }
            } else {
                self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(target_ht, pico1541_common::track_buffer::default_speed_zone(target_ht)), TrackLayout::Void, 0);
                break;
            }
        }
    }

    pub async fn run(&mut self) {
        let mut motor_on = false;
        let mut last_ht = crate::SHARED_HALF_TRACK.load(portable_atomic::Ordering::Relaxed);
        let mut last_motor = crate::SHARED_MOTOR_ON.load(portable_atomic::Ordering::Relaxed);
        loop {
            let current_ht = crate::SHARED_HALF_TRACK.load(portable_atomic::Ordering::Relaxed);
            if current_ht != last_ht {
                last_ht = current_ht;
                self.handle_track_event(current_ht).await;
                continue;
            }

            let current_motor = crate::SHARED_MOTOR_ON.load(portable_atomic::Ordering::Relaxed);
            if current_motor != last_motor {
                last_motor = current_motor;
                let active = current_motor;
                if active && !motor_on {
                    let mut disk_opt = self.disk_image.lock().await;
                    if let Some(disk) = disk_opt.as_mut() {
                        if let Some(vdir) = disk.as_virtual_disk_mut() {
                            vdir.source.rewind();
                            vdir.next_sector_to_update = 1;
                            let data = self.track_handle.start_track_modification();
                            vdir.force_update_directory_sectors(data).await;
                            self.track_handle.commit_modified_track();
                            vdir.treadmill_spinup_delay = 10;
                        }
                    }
                }
                motor_on = active;
                // Motor state is tracked in the `motor_on` variable
                continue;
            }

            let timer_fut = embassy_time::Timer::after_micros(500);
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
                                    let current_idx = self.track_handle.get_current_byte_idx();
                                    
                                    if vdir.file_storage.is_some() {
                                        // Track 1: File streaming (StriddenWarp)
                                        let data = self.track_handle.start_track_modification();
                                        if vdir.update_dynamic_buffer(current_idx, data).await {
                                            self.track_handle.commit_modified_track();
                                        }
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
                                                    let data = self.track_handle.start_track_modification();
                                                    vdir.force_update_directory_sectors(data).await;
                                                    self.track_handle.commit_modified_track();
                                                    
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

                        // Mark active track as Void during background directory fetch to prevent Core 0 emu hang/timeouts.
                        let old_layout = self.track_handle.get_active_layout();
                        self.track_handle.set_active_layout(TrackLayout::Void);

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
                                        let active_ht = self.track_handle.get_head_half_track();
                                        if active_ht == 34 || active_ht == 35 {
                                            vdir.next_sector_to_update = 1;
                                            let data = self.track_handle.start_track_modification();
                                            vdir.force_update_directory_sectors(data).await;
                                            self.track_handle.commit_modified_track();
                                            vdir.treadmill_spinup_delay = 10;
                                        }
                                    }
                                }
                            }
                        }

                        self.track_handle.set_active_layout(old_layout);
                    }
                }
                Either::Second(cmd) => {
                    match cmd {
                        InternalCommand::MotorState(_) => {}
                        

                        InternalCommand::ImageMounted => {
                            info!("Image mounted. Simulating disk insertion.");
                            
                            // 1. Unmount sequence (in case a disk was already there)
                            self.track_handle.set_active_layout(TrackLayout::Void);
                            self.track_handle.set_write_protected(true); // LOW (sliding out)
                            embassy_time::Timer::after_millis(50).await;
                            self.track_handle.set_write_protected(false); // HIGH (no disk)
                            
                            let mut disk_opt = self.disk_image.lock().await;
                            if !disk_opt.is_none() {
                                embassy_time::Timer::after_millis(400).await;
                                self.track_handle.set_write_protected(true); // LOW (sliding in)
                                embassy_time::Timer::after_millis(50).await;
                                self.track_handle.set_write_protected(true); // LOW (configured: always write protected)
                                
                                // Set force disk change flag
                                self.track_handle.set_force_disk_change(true);

                                let half_track = self.track_handle.get_head_half_track();
                                
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
                                    let data = self.track_handle.start_track_load();
                                    match disk.read_track(half_track, data).await {
                                        Ok(layout) => {
                                            self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(half_track, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(half_track))), layout.0, layout.1);
                                        }
                                        Err(e) => {
                                            error!("Failed to read track on mount: {:?}", &e);
                                            self.track_handle.set_active_layout(TrackLayout::Void);
                                        }
                                    }
                                }
                            }
                        }
                        InternalCommand::ImageUnmounted => {
                            info!("Image unmounted. Simulating disk removal.");
                            self.track_handle.set_active_layout(TrackLayout::Void);
                            self.track_handle.set_write_protected(true); // LOW (sliding out)
                            embassy_time::Timer::after_millis(50).await;
                            self.track_handle.set_write_protected(false); // HIGH (no disk)
                            
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
                                        let data = self.track_handle.start_track_modification();
                                        vdir.force_update_directory_sectors(data).await;
                                        self.track_handle.commit_modified_track();
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
                                    // Temporarily set layout to Void so the 1541 DOS waits for SYNC
                                    // while we fetch the directory, preventing race conditions and timeouts.
                                    let old_layout = self.track_handle.get_active_layout();
                                    self.track_handle.set_active_layout(TrackLayout::Void);
                                    
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
                                            let data = self.track_handle.start_track_modification();
                                            vdir.force_update_directory_sectors(data).await;
                                            self.track_handle.commit_modified_track();
                                            vdir.treadmill_spinup_delay = 10;
                                        }
                                    }
                                    self.track_handle.set_active_layout(old_layout);
                                }
                            } else {
                                info!("Opening file in Virtual Disk...");
                                // Set layout to Void while opening, to prevent reading while state is unstable
                                let old_layout = self.track_handle.get_active_layout();
                                self.track_handle.set_active_layout(TrackLayout::Void);

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
                                                let data = self.track_handle.start_track_modification();
                                                vdir.force_update_directory_sectors(data).await;
                                                self.track_handle.commit_modified_track();
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
                                                let data = self.track_handle.start_track_modification();
                                                vdir.force_update_directory_sectors(data).await;
                                                self.track_handle.commit_modified_track();
                                            }
                                        }
                                    }
                                }
                                self.track_handle.set_active_layout(old_layout);
                            }

                            let needs_regeneration = self.track_handle.get_active_layout() == TrackLayout::Void;
                            if needs_regeneration {
                                let half_track = self.track_handle.get_head_half_track();
                                let mut disk_opt = self.disk_image.lock().await;
                                if let Some(disk) = disk_opt.as_mut() {
                                    let data = self.track_handle.start_track_load();
                                    
                                    let start_time = embassy_time::Instant::now();
                                    match disk.read_track(half_track, data).await {
                                        Ok(layout) => {
                                            self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(half_track, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(half_track))), layout.0, layout.1);
                                            let elapsed = start_time.elapsed().as_millis();
                                            info!("Track load took {} ms", elapsed);
                                            if data.len() >= 16 {
                                                info!("Track {} loaded!", half_track);
                                            }
                                        }
                                        Err(_) => {
                                            self.track_handle.set_active_layout(TrackLayout::Void);
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
