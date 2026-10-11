use crate::driver::RgbLedDriver as LedsDriver;
use crate::formats::{AnyDiskImage, D64Image, G64Image, NibImage, VirtualDiskImage};
use crate::storage::{DirectoryEntry, DirectorySource, PagedDirectorySource, PAGE_CAPACITY};
use core::str::FromStr;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_io_async::{Read, Seek};
use pico1541_common::DriveLocation;
use pico1541_common::interface::TrackHandoff;
use pico1541_common::track_buffer::{ManagerHandle, TrackLayout};

pub mod cd_handler;
mod timer;
mod track;

pub use cd_handler::{
    resolve_cd_action, resolve_cd_nav, resolve_cd_path, CdAction, CdNavAction,
};

pub use file_opener::{match_c64_filename, ChildInfo, FileOpener};
pub mod file_opener;

#[inline]
fn has_extension(path: &str, ext: &str) -> bool {
    path.len() >= ext.len() && path[path.len() - ext.len()..].eq_ignore_ascii_case(ext)
}

pub fn status_led_for_location(loc: &DriveLocation) -> crate::driver::StatusLedMode {
    match loc {
        DriveLocation::Root => crate::driver::StatusLedMode::Unmounted,
        DriveLocation::VirtualDirectory { .. } => crate::driver::StatusLedMode::VirtualDisk,
        DriveLocation::DiskImage { path, .. } => {
            let p = path.as_str();
            if has_extension(p, ".g64") || has_extension(p, ".nib") {
                crate::driver::StatusLedMode::AccurateDisk
            } else {
                crate::driver::StatusLedMode::NonAccurateDisk
            }
        }
    }
}

pub struct TrackCacheTask<
    S: Read + Seek + 'static,
    O: FileOpener<FileStorage = S>,
    H: TrackHandoff,
    L: LedsDriver,
> {
    pub disk_image: Option<AnyDiskImage<S, PagedDirectorySource<144>>>,
    pub(super) track_handle: ManagerHandle,
    pub(super) opener: O,
    pub(super) handoff: H,
    pub(super) location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
    last_location: DriveLocation,
    last_motor_state: bool,
    motor_state: &'static AtomicBool,
    pub(super) just_selected: bool,
    pub(super) leds: L,
}

impl<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, H: TrackHandoff, L: LedsDriver>
    TrackCacheTask<S, O, H, L>
{
    pub fn new(
        track_handle: ManagerHandle,
        opener: O,
        handoff: H,
        location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
        motor_state: &'static AtomicBool,
        mut leds: L,
    ) -> Self {
        leds.set_status(crate::driver::StatusLedMode::Unmounted);
        Self {
            disk_image: None,
            track_handle,
            opener,
            handoff,
            location,
            last_location: DriveLocation::DiskImage {
                path: heapless::String::new(),
                use_internal: false,
            },
            last_motor_state: false,
            motor_state,
            just_selected: false,
            leds,
        }
    }

    pub async fn run(&mut self) {
        let mut heartbeat: u32 = 0;
        loop {
            heartbeat = heartbeat.wrapping_add(1);
            if heartbeat % 50 == 0 {
                #[cfg(feature = "defmt")]
                defmt::trace!("TrackCacheTask heartbeat (loop={})", heartbeat);
            }

            // Check motor state
            let current_motor_state = self.motor_state.load(Ordering::Relaxed);
            if current_motor_state && !self.last_motor_state {
                // Motor turned on
                let mut disk_opt = self.disk_image.as_mut();
                if let Some(disk) = disk_opt.as_mut() {
                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                        vdir.source.rewind();
                        let data = self.track_handle.start_track_modification();
                        vdir.force_update_directory_sectors(data).await;
                        self.track_handle.commit_modified_track();
                    }
                }
            }
            self.last_motor_state = current_motor_state;

            // Check location changes
            let current_loc = self.location.lock().await.clone();
            if current_loc != self.last_location {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "TrackCacheTask detected location change: {:?}",
                    current_loc
                );

                match &current_loc {
                    DriveLocation::Root => {
                        self.mount_root().await;
                    }
                    DriveLocation::VirtualDirectory { path, use_internal } => {
                        self.mount_sequence(path.clone(), *use_internal).await;
                    }
                    DriveLocation::DiskImage { path, use_internal } => {
                        self.mount_sequence(path.clone(), *use_internal).await;
                    }
                }
                self.leds.set_status(status_led_for_location(&current_loc));
                self.last_location = current_loc;
            }

            if PAGE_CHANGED.swap(false, Ordering::AcqRel) {
                let page = CURRENT_PAGE.load(Ordering::Acquire);
                self.refresh_paged_directory(page).await;
            }

            if SLOT_SELECTED.swap(false, Ordering::AcqRel) {
                let slot = SELECTED_SLOT.load(Ordering::Acquire);
                self.select_virtual_file(slot).await;
                continue;
            }

            let track_fut = self.handoff.wait_for_change();
            let timer_fut = embassy_time::Timer::after_millis(20);

            match select(track_fut, timer_fut).await {
                Either::First(half_track) => {
                    self.handle_track_event(half_track).await;
                }
                Either::Second(_) => {
                    self.handle_timer_event(self.last_motor_state).await;
                }
            }
        }
    }

    #[allow(dead_code)]
    async fn unmount_sequence(&mut self) {
        #[cfg(feature = "defmt")]
        defmt::info!("Image unmounted. Simulating disk removal.");
        self.disk_image = None;
        self.track_handle.set_active_layout(TrackLayout::Void);
        self.track_handle.set_write_protected(true); // LOW (sliding out)
        embassy_time::Timer::after_millis(50).await;
        self.track_handle.set_write_protected(false); // HIGH (no disk)
        embassy_time::Timer::after_millis(50).await;
    }

    async fn simulate_disk_swap(&mut self, final_write_protected: bool) {
        #[cfg(feature = "defmt")]
        defmt::info!("Simulating physical disk swap (Write Protect sensor toggle).");

        self.track_handle.set_active_layout(TrackLayout::Void);
        self.track_handle.set_write_protected(true); // LOW (sliding out)
        embassy_time::Timer::after_millis(50).await;
        self.track_handle.set_write_protected(false); // HIGH (no disk, phototransistor uncovered)
        embassy_time::Timer::after_millis(300).await;
        self.track_handle.set_write_protected(true); // LOW (sliding in, phototransistor covered)
        embassy_time::Timer::after_millis(50).await;
        self.track_handle.set_write_protected(final_write_protected);
        self.track_handle.force_disk_change();
    }

    async fn mount_root(&mut self) {
        let mut dir_source = PagedDirectorySource::<144>::new(b"STORAGE");
        dir_source.set_page_entries(
            0,
            &[
                DirectoryEntry::new_dir(b"INTERNAL", 17, 0),
                DirectoryEntry::new_dir(b"SD CARD", 17, 1),
            ],
            false,
        );
        let vdisk = VirtualDiskImage::new(dir_source, b"STORAGE");
        self.disk_image = Some(AnyDiskImage::VirtualDisk(vdisk));
        {
            let mut guard = ACTIVE_CHILDREN.lock().await;
            guard.clear();
        }

        #[cfg(feature = "defmt")]
        defmt::info!("Root directory mounted.");

        self.simulate_disk_swap(true).await;

        let half_track = self.track_handle.get_head_half_track();
        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                vdir.source.rewind();
            }
        }
        self.handle_track_event(half_track).await;
    }

    #[allow(unused_variables, unused_assignments)]
    async fn mount_sequence(&mut self, path: heapless::String<128>, use_internal: bool) {
        let p_str = path.as_str();

        let is_d64 = has_extension(p_str, ".d64");
        let is_g64 = has_extension(p_str, ".g64");
        let is_nib = has_extension(p_str, ".nib");

        let ht1 = self.track_handle.get_head_half_track();
        let mut ht2 = ht1;

        if is_d64 || is_g64 || is_nib {
            {
                let mut guard = ACTIVE_CHILDREN.lock().await;
                guard.clear();
            }
            let file = if use_internal {
                self.opener.open_internal_file(p_str)
            } else {
                self.opener.open_sd_file(p_str)
            };

            if is_d64 {
                let d64 = D64Image::new(file).await;
                self.disk_image = Some(AnyDiskImage::D64(d64));
            } else if is_g64 {
                self.disk_image = G64Image::from_storage(file)
                    .await
                    .ok()
                    .map(AnyDiskImage::G64);
                ht2 = self.track_handle.get_head_half_track();
            } else if is_nib {
                self.disk_image = NibImage::from_storage(file)
                    .await
                    .ok()
                    .map(AnyDiskImage::Nib);
                ht2 = self.track_handle.get_head_half_track();
            }
        } else if is_single_file(p_str) {
            // ── Single non-image file (PRG, SEQ, …) ──────────────────────────────
            // Build a VirtualDisk with one shortcut directory entry (track=1,
            // sector=1) and attach the opened file as file_storage.  The track-1
            // treadmill in update_dynamic_buffer then streams the file data to the
            // 1541 DOS as the disk rotates.

            let name_bytes = last_segment(p_str).as_bytes();

            let dir_source = PagedDirectorySource::<144>::new(name_bytes);
            let mut vdisk = VirtualDiskImage::new(dir_source, name_bytes);

            // Open the file and attach it as the file storage.
            let mut file = if use_internal {
                self.opener.open_internal_file(p_str)
            } else {
                self.opener.open_sd_file(p_str)
            };

            // Query file size so directory entry shows accurate block count
            let file_size = embedded_io_async::Seek::seek(&mut file, embedded_io_async::SeekFrom::End(0))
                .await
                .unwrap_or(0);
            let _ = embedded_io_async::Seek::seek(&mut file, embedded_io_async::SeekFrom::Start(0)).await;

            // Single directory entry: filename + first_track=17, first_sector=0.
            let entry = DirectoryEntry::from_file_info(name_bytes, false, file_size as u32);
            vdisk.set_shortcut_entry(Some(entry));
            vdisk.file_storage = Some(file);

            self.disk_image = Some(AnyDiskImage::VirtualDisk(vdisk));

            #[cfg(feature = "defmt")]
            defmt::info!("PRG mount: \"{}\"", p_str);
        } else {
            // ── Virtual directory listing ─────────────────────────────────────────
            let mut name_bytes = b"SD CARD".as_slice();
            if p_str == "/" || p_str.is_empty() || (use_internal && p_str == "/files") {
                if use_internal {
                    name_bytes = b"INTERNAL".as_slice();
                } else {
                    name_bytes = b"SD CARD".as_slice();
                }
            } else if let Some(idx) = p_str.rfind('/') {
                name_bytes = p_str[idx + 1..].as_bytes();
            } else {
                name_bytes = p_str.as_bytes();
            }

            let dir_source = PagedDirectorySource::<144>::new(name_bytes);
            let vdisk = VirtualDiskImage::new(dir_source, name_bytes);
            self.disk_image = Some(AnyDiskImage::VirtualDisk(vdisk));

            let mut count = 0;
            let mut has_more = false;
            let mut active_children_batch = heapless::Vec::<CachedChildEntry, 48>::new();
            let disk_image = &mut self.disk_image;
            let _ = self
                .opener
                .list_dir_range(
                    p_str,
                    use_internal,
                    0,
                    PAGE_CAPACITY + 1,
                    |entry, name_str| {
                        if count < PAGE_CAPACITY {
                            if let Some(disk) = disk_image.as_mut() {
                                if let Some(vdir) = disk.as_virtual_disk_mut() {
                                    if let Some(paged) = vdir.source.as_paged_mut() {
                                        paged.set_page_entry_with_name(count, entry, name_str);
                                    }
                                }
                            }
                            let mut s = heapless::String::<64>::new();
                            let _ = s.push_str(name_str);
                            let _ = active_children_batch.push(CachedChildEntry {
                                petscii_name: entry.filename,
                                real_name: s,
                                is_dir: entry.is_dir(),
                            });
                        } else {
                            has_more = true;
                        }
                        count += 1;
                    },
                )
                .await;
            {
                let mut guard = ACTIVE_CHILDREN.lock().await;
                *guard = active_children_batch;
            }

            #[cfg(feature = "defmt")]
            defmt::info!(
                "Initial dir fetch: {} entries for \"{}\"",
                count.min(PAGE_CAPACITY),
                p_str
            );

            if let Some(disk) = self.disk_image.as_mut() {
                if let Some(vdir) = disk.as_virtual_disk_mut() {
                    if let Some(paged) = vdir.source.as_paged_mut() {
                        let loaded_count = count.min(PAGE_CAPACITY);
                        for i in loaded_count..PAGE_CAPACITY {
                            paged.set_page_entry(i, DirectoryEntry::default());
                        }
                        if count < PAGE_CAPACITY
                            || (count == PAGE_CAPACITY && !has_more)
                        {
                            paged.set_total_count(count);
                        } else if has_more {
                            let known_min = PAGE_CAPACITY + 1;
                            if paged.total_count() == usize::MAX || paged.total_count() < known_min {
                                paged.set_total_count(known_min);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Disk Image mounted: {}. Head: {}, {}",
            p_str,
            ht1,
            ht2
        );

        let final_wp = if let Some(disk) = self.disk_image.as_mut() {
            disk.as_virtual_disk_mut().is_some()
        } else {
            false
        };

        self.simulate_disk_swap(final_wp).await;

        let mut disk_opt = self.disk_image.as_mut();
        if let Some(disk) = disk_opt.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                vdir.source.rewind();
            }
            let half_track = self.track_handle.get_head_half_track();
            drop(disk_opt);

            self.handle_track_event(half_track).await;
        }
    }

    async fn refresh_paged_directory(&mut self, page: usize) {
        let (current_path, current_use_internal) = match *self.location.lock().await {
            DriveLocation::VirtualDirectory { ref path, use_internal } => (path.clone(), use_internal),
            _ => return,
        };
        let p_str = current_path.as_str();

        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                if let Some(paged) = vdir.source.as_paged_mut() {
                    paged.set_page(page);
                }
            }
        }

        let start_entry = page * PAGE_CAPACITY;
        let mut count = 0;
        let mut has_more = false;
        let mut active_children_batch = heapless::Vec::<CachedChildEntry, 48>::new();
        let disk_image = &mut self.disk_image;
        let _ = self
            .opener
            .list_dir_range(
                p_str,
                current_use_internal,
                start_entry,
                PAGE_CAPACITY + 1,
                |entry, name_str| {
                    if count < PAGE_CAPACITY {
                        if let Some(disk) = disk_image.as_mut() {
                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                if let Some(paged) = vdir.source.as_paged_mut() {
                                    paged.set_page_entry_with_name(count, entry, name_str);
                                }
                            }
                        }
                        let mut s = heapless::String::<64>::new();
                        let _ = s.push_str(name_str);
                        let _ = active_children_batch.push(CachedChildEntry {
                            petscii_name: entry.filename,
                            real_name: s,
                            is_dir: entry.is_dir(),
                        });
                    } else {
                        has_more = true;
                    }
                    count += 1;
                },
            )
            .await;
        {
            let mut guard = ACTIVE_CHILDREN.lock().await;
            *guard = active_children_batch;
        }

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Page refresh: {} entries for \"{}\", page {}",
            count.min(PAGE_CAPACITY),
            p_str,
            page
        );

        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                if let Some(paged) = vdir.source.as_paged_mut() {
                    let loaded_count = count.min(PAGE_CAPACITY);
                    for i in loaded_count..PAGE_CAPACITY {
                        paged.set_page_entry(i, DirectoryEntry::default());
                    }
                    if count < PAGE_CAPACITY
                        || (count == PAGE_CAPACITY && !has_more)
                    {
                        paged.set_total_count(start_entry + count);
                    } else if has_more {
                        let known_min = (page + 1) * PAGE_CAPACITY + 1;
                        if paged.total_count() == usize::MAX || paged.total_count() < known_min {
                            paged.set_total_count(known_min);
                        }
                    }
                }
            }
        }

        self.simulate_disk_swap(true).await;

        let half_track = self.track_handle.get_head_half_track();
        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                vdir.source.rewind();
            }
        }
        self.handle_track_event(half_track).await;
    }

    pub async fn select_virtual_file(&mut self, slot: usize) {
        let current_loc = self.location.lock().await.clone();
        match current_loc {
            DriveLocation::Root => {
                if slot == 0 {
                    CURRENT_PAGE.store(0, Ordering::Release);
                    *self.location.lock().await = DriveLocation::VirtualDirectory {
                        path: heapless::String::try_from("/files").unwrap_or_default(),
                        use_internal: true,
                    };
                    crate::LOCATION_CHANGED.signal(());
                } else if slot == 1 {
                    CURRENT_PAGE.store(0, Ordering::Release);
                    *self.location.lock().await = DriveLocation::VirtualDirectory {
                        path: heapless::String::try_from("/").unwrap_or_default(),
                        use_internal: false,
                    };
                    crate::LOCATION_CHANGED.signal(());
                }
            }
            DriveLocation::VirtualDirectory { .. } => {
                if let Some(disk) = self.disk_image.as_mut() {
                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                        if vdir.file_storage.is_some() {
                            return;
                        }

                        let entry_opt = if let Some(paged) = vdir.source.as_paged_mut() {
                            paged.entry_at_slot(slot)
                        } else {
                            None
                        };

                        if let Some(entry) = entry_opt {
                            if entry.is_dir() {
                                let (name, _) = if let Some(paged) = vdir.source.as_paged_mut() {
                                    paged.child_at_slot(slot).unwrap_or_default()
                                } else {
                                    (heapless::String::new(), true)
                                };
                                if let Some(target_path) = cd_handler::go_to_child(current_loc.path(), name.as_str()) {
                                    CURRENT_PAGE.store(0, Ordering::Release);
                                    *self.location.lock().await = DriveLocation::VirtualDirectory {
                                        path: target_path,
                                        use_internal: current_loc.use_internal(),
                                    };
                                    crate::LOCATION_CHANGED.signal(());
                                }
                            } else if entry.filename[0] != 0 {
                                if let Ok(file) = self.opener.open_file(&entry.filename).await {
                                    vdir.set_prg_start(entry.first_track, entry.first_sector);
                                    vdir.file_storage = Some(file);
                                    #[cfg(feature = "defmt")]
                                    defmt::info!("Mounted virtual file for slot {}", slot);
                                    log::info!("Mounted virtual file for slot {}", slot);

                                    let ht = self.track_handle.get_head_half_track();
                                    if ht < 34 {
                                        self.handle_track_event(ht).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            DriveLocation::DiskImage { .. } => {}
        }
    }

    pub fn resolve_child_in_memory(&mut self, child_name: &str) -> Option<(heapless::String<64>, bool)> {
        let mut petscii_buf = [0xA0; 16];
        for (i, &b) in child_name.as_bytes().iter().take(16).enumerate() {
            petscii_buf[i] = crate::storage::ascii_to_petscii(b);
        }
        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                if let Some(paged) = vdir.source.as_paged_mut() {
                    return paged.resolve_child(&petscii_buf);
                }
            }
        }
        None
    }
}

// ── Path helpers ──────────────────────────────────────────────────────────────

/// Returns `true` when `path` has a file extension that is **not** one of the
/// recognised disk-image formats (.d64 / .g64 / .nib).  Directories (no `.`
/// after the last `/`) and image files both return `false`.
fn is_single_file(path: &str) -> bool {
    let seg = last_segment(path);
    if let Some(dot) = seg.rfind('.') {
        let ext = &seg[dot..];
        !ext.eq_ignore_ascii_case(".d64")
            && !ext.eq_ignore_ascii_case(".g64")
            && !ext.eq_ignore_ascii_case(".nib")
    } else {
        false
    }
}

/// Returns the last path segment (filename) without a leading slash.
fn last_segment(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) => &path[idx + 1..],
        None => path,
    }
}

use embassy_sync::channel::Receiver;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedChildEntry {
    pub petscii_name: [u8; 16],
    pub real_name: heapless::String<64>,
    pub is_dir: bool,
}

pub static ACTIVE_CHILDREN: Mutex<CriticalSectionRawMutex, heapless::Vec<CachedChildEntry, 48>> =
    Mutex::new(heapless::Vec::new());

pub async fn resolve_active_child(child_name: &str) -> Option<(heapless::String<64>, bool)> {
    let mut petscii_buf = [0xA0; 16];
    for (i, &b) in child_name.as_bytes().iter().take(16).enumerate() {
        petscii_buf[i] = crate::storage::ascii_to_petscii(b);
    }
    let guard = ACTIVE_CHILDREN.lock().await;
    for entry in guard.iter() {
        if file_opener::match_c64_filename(&entry.petscii_name, &petscii_buf) {
            return Some((entry.real_name.clone(), entry.is_dir));
        }
    }
    None
}

pub static CURRENT_PAGE: portable_atomic::AtomicUsize = portable_atomic::AtomicUsize::new(0);
pub static PAGE_CHANGED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
pub static SELECTED_SLOT: portable_atomic::AtomicUsize = portable_atomic::AtomicUsize::new(usize::MAX);
pub static SLOT_SELECTED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);

pub struct MountControllerTask {
    location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
    motor_state: &'static AtomicBool,
    cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
}

impl MountControllerTask {
    pub fn new(
        location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
        motor_state: &'static AtomicBool,
        cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
    ) -> Self {
        Self {
            location,
            motor_state,
            cmd_rx,
        }
    }

    pub fn current_page(&self) -> usize {
        CURRENT_PAGE.load(Ordering::Relaxed)
    }

    pub async fn run(&mut self) {
        loop {
            let cmd = self.cmd_rx.receive().await;
            match cmd {
                crate::InternalCommand::MotorState(active) => {
                    self.motor_state.store(active, Ordering::Relaxed);
                }
                crate::InternalCommand::Mount(payload) => {
                    CURRENT_PAGE.store(0, Ordering::Release);
                    let new_loc = match payload {
                        crate::MountPayload::VirtualDirectory(p, i) => DriveLocation::VirtualDirectory {
                            path: p,
                            use_internal: i,
                        },
                        crate::MountPayload::Prg(p, i) => {
                            let parent = match p.rfind('/') {
                                Some(0) => "/",
                                Some(idx) => &p[..idx],
                                None => "/",
                            };
                            DriveLocation::VirtualDirectory {
                                path: heapless::String::from_str(parent).unwrap_or_default(),
                                use_internal: i,
                            }
                        }
                        crate::MountPayload::D64(p, i)
                        | crate::MountPayload::G64(p, i)
                        | crate::MountPayload::Nib(p, i) => DriveLocation::DiskImage {
                            path: p,
                            use_internal: i,
                        },
                    };
                    *self.location.lock().await = new_loc;
                    crate::LOCATION_CHANGED.signal(());
                }
                crate::InternalCommand::Unmount => {
                    CURRENT_PAGE.store(0, Ordering::Release);
                    *self.location.lock().await = DriveLocation::Root;
                    crate::LOCATION_CHANGED.signal(());
                }
                crate::InternalCommand::Cd(cd_str) => {
                    let current_loc = self.location.lock().await.clone();
                    match current_loc {
                        DriveLocation::Root => {
                            let nav = cd_handler::resolve_cd_nav(&current_loc, cd_str.as_str());
                            if let cd_handler::CdNavAction::Transition(loc) = nav {
                                CURRENT_PAGE.store(0, Ordering::Release);
                                *self.location.lock().await = loc;
                                crate::LOCATION_CHANGED.signal(());
                            }
                        }
                        DriveLocation::VirtualDirectory { ref path, use_internal } => {
                            let nav = cd_handler::resolve_cd_nav(&current_loc, cd_str.as_str());
                            match nav {
                                cd_handler::CdNavAction::NextPage => {
                                    let next = CURRENT_PAGE.load(Ordering::Acquire) + 1;
                                    CURRENT_PAGE.store(next, Ordering::Release);
                                    PAGE_CHANGED.store(true, Ordering::Release);
                                }
                                cd_handler::CdNavAction::PrevPage => {
                                    let prev = CURRENT_PAGE.load(Ordering::Acquire).saturating_sub(1);
                                    CURRENT_PAGE.store(prev, Ordering::Release);
                                    PAGE_CHANGED.store(true, Ordering::Release);
                                }
                                cd_handler::CdNavAction::Transition(new_loc) => {
                                    let child_seg = cd_handler::extract_cd_target(cd_str.as_str());
                                    let resolved_child = resolve_active_child(child_seg).await;
                                    let target_loc = match (new_loc, resolved_child) {
                                        (DriveLocation::Root, _) => DriveLocation::Root,
                                        (DriveLocation::VirtualDirectory { path: p, .. }, Some((name, true))) => {
                                            let next_p = cd_handler::go_to_child(path.as_str(), name.as_str()).unwrap_or(p);
                                            DriveLocation::VirtualDirectory { path: next_p, use_internal }
                                        }
                                        (DriveLocation::VirtualDirectory { path: p, .. }, Some((name, false))) => {
                                            if cd_handler::is_disk_image(name.as_str()) {
                                                let next_p = cd_handler::go_to_child(path.as_str(), name.as_str()).unwrap_or(p);
                                                DriveLocation::DiskImage { path: next_p, use_internal }
                                            } else {
                                                DriveLocation::VirtualDirectory { path: p, use_internal }
                                            }
                                        }
                                        (DriveLocation::DiskImage { path: p, .. }, Some((name, false))) => {
                                            let next_p = cd_handler::go_to_child(path.as_str(), name.as_str()).unwrap_or(p);
                                            DriveLocation::DiskImage { path: next_p, use_internal }
                                        }
                                        (other, _) => other,
                                    };
                                    CURRENT_PAGE.store(0, Ordering::Release);
                                    *self.location.lock().await = target_loc;
                                    crate::LOCATION_CHANGED.signal(());
                                }
                                cd_handler::CdNavAction::Ignore => {}
                            }
                        }
                        DriveLocation::DiskImage { .. } => {
                            let nav = cd_handler::resolve_cd_nav(&current_loc, cd_str.as_str());
                            if let cd_handler::CdNavAction::Transition(loc) = nav {
                                CURRENT_PAGE.store(0, Ordering::Release);
                                *self.location.lock().await = loc;
                                crate::LOCATION_CHANGED.signal(());
                            }
                        }
                    }
                }
                crate::InternalCommand::SelectVirtualFile(slot) => {
                    #[cfg(feature = "defmt")]
                    defmt::debug!("SelectVirtualFile received for slot {}", slot);
                    log::debug!("SelectVirtualFile received for slot {}", slot);
                    SELECTED_SLOT.store(slot, Ordering::Release);
                    SLOT_SELECTED.store(true, Ordering::Release);
                }
            }
        }
    }
}
