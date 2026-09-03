use crate::formats::DiskImage;
use super::{MountManagerTask, FileOpener};
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;
use crate::{InternalCommand, MountPayload};
use crate::storage::DirectorySource;

#[cfg(feature = "defmt")]
use defmt::{info, warn, error};
#[cfg(not(feature = "defmt"))]
use log::{info, warn, error};

impl<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, L: crate::driver::RgbLedDriver> MountManagerTask<S, O, L> {
    pub(super) async fn handle_cmd_event(&mut self, cmd: InternalCommand, motor_on: &mut bool) {
        match cmd {
            InternalCommand::MotorState(active) => {
                if active && !*motor_on {
                    let mut disk_opt = self.disk_image.as_mut();
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
                *motor_on = active;
            }
            
            InternalCommand::Mount(payload) => {

                let (path, use_internal, led_mode, ht1, ht2) = match payload {
                    MountPayload::VirtualDirectory(p, i) => {
                        let dir_source = crate::storage::PagedDirectorySource::<144>::new(p.as_bytes());
                        let vdisk = crate::formats::VirtualDiskImage::new(dir_source, p.as_bytes());
                        self.disk_image = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
                        (p, i, crate::driver::StatusLedMode::VirtualDisk, self.track_handle.get_head_half_track(), self.track_handle.get_head_half_track())
                    }
                    MountPayload::D64(p, i) => {
                        let file = if i {
                            self.opener.open_internal_file(p.as_str())
                        } else {
                            self.opener.open_sd_file(p.as_str())
                        };
                        let d64 = crate::formats::D64Image::new(file);
                        self.disk_image = Some(crate::formats::AnyDiskImage::D64(d64));
                        (p, i, crate::driver::StatusLedMode::NonAccurateDisk, self.track_handle.get_head_half_track(), self.track_handle.get_head_half_track())
                    }
                    MountPayload::G64(p, i) => {
                        let file = if i {
                            self.opener.open_internal_file(p.as_str())
                        } else {
                            self.opener.open_sd_file(p.as_str())
                        };
                        let ht1 = self.track_handle.get_head_half_track();
                        let ht2: u8;
                        
                        if let Ok(g64) = crate::formats::G64Image::from_storage(file).await {
                            ht2 = self.track_handle.get_head_half_track();
                            self.disk_image = Some(crate::formats::AnyDiskImage::G64(g64));
                            (p, i, crate::driver::StatusLedMode::AccurateDisk, ht1, ht2)
                        } else {
                            ht2 = self.track_handle.get_head_half_track();
                            self.disk_image = None;
                            (p, i, crate::driver::StatusLedMode::Unmounted, ht1, ht2)
                        }
                    }
                };

                {
                    let mut guard = self.mounted_dir.lock().await;
                    guard.0 = path.clone();
                    guard.1 = use_internal;
                }
                self.leds.set_status(led_mode);

                info!("Disk Image mounted: {}. Simulating physical disk swap. Head: {}, {}", path.as_str(), ht1, ht2);
                
                // 1. Unmount sequence (in case a disk was already there)
                self.track_handle.set_active_layout(TrackLayout::Void);
                self.track_handle.set_write_protected(true); // LOW (sliding out)
                embassy_time::Timer::after_millis(50).await;
                self.track_handle.set_write_protected(false); // HIGH (no disk)
                
                let mut disk_opt = self.disk_image.as_mut();
                if !disk_opt.is_none() {
                    embassy_time::Timer::after_millis(400).await;
                    self.track_handle.set_write_protected(true); // LOW (sliding in)
                    embassy_time::Timer::after_millis(50).await;
                    self.track_handle.set_write_protected(true); // LOW (configured: always write protected)
                    
                    // Set force disk change flag
                    self.track_handle.set_force_disk_change(true);

                    let half_track = self.track_handle.get_head_half_track();
                    
                    // Initialize Virtual Directory if applicable before reading track
                    let mut _is_virtual_disk = false;
                    if let Some(disk) = disk_opt.as_mut() {
                        if let Some(vdir) = disk.as_virtual_disk_mut() {
                            _is_virtual_disk = true;
                            vdir.set_shortcut_entry(None);
                            vdir.file_storage = None;
                            vdir.source.rewind();
                            if let Some(paged) = vdir.source.as_paged_mut() {
                                paged.invalidate();
                            }
                        }
                    }
                    drop(disk_opt);
                    
                    if false /* is_virtual_disk */ {
                        let (dir_path, use_internal) = {
                            let guard = self.mounted_dir.lock().await;
                            (guard.0.clone(), guard.1)
                        };
                        
                        let mut all_entries = heapless::Vec::<crate::storage::DirectoryEntry, 48>::new();
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

                        let mut disk_opt = self.disk_image.as_mut();
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

                    let mut disk_opt = self.disk_image.as_mut();
                    if let Some(disk) = disk_opt.as_mut() {
                        let data = self.track_handle.start_track_load();
                        let start_time = embassy_time::Instant::now();
                        match disk.read_track(half_track, data).await {
                            Ok(layout) => {
                                self.track_handle.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(half_track, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(half_track))), layout.0, layout.1);
                                let elapsed = start_time.elapsed().as_millis();
                                info!("Track {}.{} (HT: {}) load took {} ms (2)", (half_track / 2) + 1, half_track % 2, half_track, elapsed);
                            }
                            Err(e) => {
                                error!("Failed to read track on mount: {:?}", &e);
                                self.track_handle.set_active_layout(TrackLayout::Void);
                            }
                        }
                    }
                }
            }
            InternalCommand::Unmount => {
                info!("Image unmounted. Simulating disk removal.");
                self.disk_image = None;
                {
                    let mut guard = self.mounted_dir.lock().await;
                    guard.0 = heapless::String::new();
                    guard.1 = false;
                }
                self.leds.set_status(crate::driver::StatusLedMode::Unmounted);
                self.track_handle.set_active_layout(TrackLayout::Void);
                self.track_handle.set_write_protected(true); // LOW (sliding out)
                embassy_time::Timer::after_millis(50).await;
                self.track_handle.set_write_protected(false); // HIGH (no disk)
                
                // Wait a bit for the C64 DOS to process the WP change before logging the final state
                embassy_time::Timer::after_millis(50).await;
            }
            InternalCommand::CloseChannel(sa) => {
                if sa == 0 {
                    info!("Channel 0 closed. Rewinding virtual directory.");
                    let mut disk_opt = self.disk_image.as_mut();
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
                    let mut _is_virtual_disk = false;
                    let mut disk_opt = self.disk_image.as_mut();
                    if let Some(disk) = disk_opt.as_mut() {
                        if let Some(vdir) = disk.as_virtual_disk_mut() {
                            _is_virtual_disk = true;
                            vdir.set_shortcut_entry(None);
                            vdir.file_storage = None;
                            vdir.source.rewind();
                            if let Some(paged) = vdir.source.as_paged_mut() {
                                paged.invalidate();
                            }
                        }
                    }
                    drop(disk_opt);
                    
                    if false /* _is_virtual_disk */ {
                        // Temporarily set layout to Void so the 1541 DOS waits for SYNC
                        // while we fetch the directory, preventing race conditions and timeouts.
                        let old_layout = self.track_handle.get_active_layout();
                        self.track_handle.set_active_layout(TrackLayout::Void);
                        
                        // Fetch first block synchronously to prevent empty directory
                        let (dir_path, use_internal) = {
                            let guard = self.mounted_dir.lock().await;
                            (guard.0.clone(), guard.1)
                        };
                        
                        let mut all_entries = heapless::Vec::<crate::storage::DirectoryEntry, 48>::new();
                        let _ = self.opener.list_dir_range(
                            dir_path.as_str(),
                            use_internal,
                            0,
                            16,
                            |entry| {
                                let _ = all_entries.push(entry);
                            }
                        ).await;
                        
                        let mut disk_opt = self.disk_image.as_mut();
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
                            
                            let shortcut = crate::storage::DirectoryEntry::new(&entry_name, 0x82, 1, 1);
                            let mut disk_opt = self.disk_image.as_mut();
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
                            let mut disk_opt = self.disk_image.as_mut();
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
                    let mut disk_opt = self.disk_image.as_mut();
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
