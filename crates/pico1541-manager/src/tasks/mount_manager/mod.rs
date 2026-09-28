use crate::driver::RgbLedDriver as LedsDriver;
use crate::formats::AnyDiskImage;
use crate::storage::DirectorySource;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_io_async::{Read, Seek};
use pico1541_common::interface::TrackHandoff;
use pico1541_common::track_buffer::{ManagerHandle, TrackLayout};

pub mod cd_handler;
mod timer;
mod track;

pub use cd_handler::{resolve_cd_action, resolve_cd_path, CdAction};

pub use file_opener::{match_c64_filename, FileOpener};
pub mod file_opener;

pub struct TrackCacheTask<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, H: TrackHandoff>
{
    pub(super) disk_image: Option<AnyDiskImage<S, crate::storage::PagedDirectorySource<144>>>,
    pub(super) track_handle: ManagerHandle,
    pub(super) opener: O,
    pub(super) handoff: H,
    pub(super) mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    last_mounted_path: heapless::String<128>,
    last_motor_state: bool,
    motor_state: &'static AtomicBool,
    pub(super) just_selected: bool,
}

impl<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, H: TrackHandoff>
    TrackCacheTask<S, O, H>
{
    pub fn new(
        track_handle: ManagerHandle,
        opener: O,
        handoff: H,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        motor_state: &'static AtomicBool,
    ) -> Self {
        Self {
            disk_image: None,
            track_handle,
            opener,
            handoff,
            mounted_dir,
            last_mounted_path: heapless::String::new(),
            last_motor_state: false,
            motor_state,
            just_selected: false,
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

            // Check mounted dir changes
            let (current_path, current_use_internal) = {
                let guard = self.mounted_dir.lock().await;
                (guard.0.clone(), guard.1)
            };

            if current_path != self.last_mounted_path {
                #[cfg(feature = "defmt")]
                defmt::info!(
                    "TrackCacheTask detected path change: {} -> {}",
                    self.last_mounted_path.as_str(),
                    current_path.as_str()
                );

                if current_path.is_empty() {
                    self.unmount_sequence().await;
                } else {
                    self.mount_sequence(current_path.clone(), current_use_internal)
                        .await;
                }
                self.last_mounted_path = current_path;
            }

            if PAGE_CHANGED.swap(false, Ordering::AcqRel) {
                let page = CURRENT_PAGE.load(Ordering::Acquire);
                self.refresh_paged_directory(page).await;
            }

            if SLOT_SELECTED.swap(false, Ordering::AcqRel) {
                let slot = SELECTED_SLOT.load(Ordering::Acquire);
                self.select_virtual_file(slot).await;
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

    #[allow(unused_variables, unused_assignments)]
    async fn mount_sequence(&mut self, path: heapless::String<128>, use_internal: bool) {
        let p_str = path.as_str();

        let is_d64 = p_str.ends_with(".d64") || p_str.ends_with(".D64");
        let is_g64 = p_str.ends_with(".g64") || p_str.ends_with(".G64");
        let is_nib = p_str.ends_with(".nib") || p_str.ends_with(".NIB");

        let ht1 = self.track_handle.get_head_half_track();
        let mut ht2 = ht1;

        if is_d64 || is_g64 || is_nib {
            let file = if use_internal {
                self.opener.open_internal_file(p_str)
            } else {
                self.opener.open_sd_file(p_str)
            };

            if is_d64 {
                let d64 = crate::formats::D64Image::new(file).await;
                self.disk_image = Some(crate::formats::AnyDiskImage::D64(d64));
            } else if is_g64 {
                self.disk_image = crate::formats::G64Image::from_storage(file)
                    .await
                    .ok()
                    .map(crate::formats::AnyDiskImage::G64);
                ht2 = self.track_handle.get_head_half_track();
            } else if is_nib {
                self.disk_image = crate::formats::NibImage::from_storage(file)
                    .await
                    .ok()
                    .map(crate::formats::AnyDiskImage::Nib);
                ht2 = self.track_handle.get_head_half_track();
            }
        } else if is_single_file(p_str) {
            // ── Single non-image file (PRG, SEQ, …) ──────────────────────────────
            // Build a VirtualDisk with one shortcut directory entry (track=1,
            // sector=1) and attach the opened file as file_storage.  The track-1
            // treadmill in update_dynamic_buffer then streams the file data to the
            // 1541 DOS as the disk rotates.

            let name_bytes = last_segment(p_str).as_bytes();

            let dir_source = crate::storage::PagedDirectorySource::<144>::new(name_bytes);
            let mut vdisk = crate::formats::VirtualDiskImage::new(dir_source, name_bytes);

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
            let entry = crate::storage::DirectoryEntry::from_file_info(name_bytes, false, file_size as u32);
            vdisk.set_shortcut_entry(Some(entry));
            vdisk.file_storage = Some(file);

            self.disk_image = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));

            #[cfg(feature = "defmt")]
            defmt::info!("PRG mount: \"{}\"", p_str);
        } else {
            // ── Virtual directory listing ─────────────────────────────────────────
            let mut name_bytes = b"SD CARD".as_slice();
            if p_str == "/" || p_str.is_empty() {
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

            let dir_source = crate::storage::PagedDirectorySource::<144>::new(name_bytes);
            let vdisk = crate::formats::VirtualDiskImage::new(dir_source, name_bytes);
            self.disk_image = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));

            let mut count = 0;
            let mut has_more = false;
            let disk_image = &mut self.disk_image;
            let _ = self
                .opener
                .list_dir_range(
                    p_str,
                    use_internal,
                    0,
                    crate::storage::PAGE_CAPACITY + 1,
                    |entry| {
                        if count < crate::storage::PAGE_CAPACITY {
                            if let Some(disk) = disk_image.as_mut() {
                                if let Some(vdir) = disk.as_virtual_disk_mut() {
                                    if let Some(paged) = vdir.source.as_paged_mut() {
                                        paged.set_page_entry(count, entry);
                                    }
                                }
                            }
                        } else {
                            has_more = true;
                        }
                        count += 1;
                    },
                )
                .await;

            #[cfg(feature = "defmt")]
            defmt::info!(
                "Initial dir fetch: {} entries for \"{}\"",
                count.min(crate::storage::PAGE_CAPACITY),
                p_str
            );

            if let Some(disk) = self.disk_image.as_mut() {
                if let Some(vdir) = disk.as_virtual_disk_mut() {
                    if let Some(paged) = vdir.source.as_paged_mut() {
                        let loaded_count = count.min(crate::storage::PAGE_CAPACITY);
                        for i in loaded_count..crate::storage::PAGE_CAPACITY {
                            paged.set_page_entry(i, crate::storage::DirectoryEntry::default());
                        }
                        if count < crate::storage::PAGE_CAPACITY
                            || (count == crate::storage::PAGE_CAPACITY && !has_more)
                        {
                            paged.set_total_count(count);
                        } else if has_more {
                            let known_min = crate::storage::PAGE_CAPACITY + 1;
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
            "Disk Image mounted: {}. Simulating physical disk swap. Head: {}, {}",
            p_str,
            ht1,
            ht2
        );

        // 1. Unmount sequence
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

            self.track_handle.set_force_disk_change(true);

            let half_track = self.track_handle.get_head_half_track();

            if let Some(disk) = disk_opt.as_mut() {
                if let Some(vdir) = disk.as_virtual_disk_mut() {
                    // NOTE: do NOT clear shortcut_entry or file_storage here.
                    // mount_sequence always creates a fresh VirtualDiskImage, so these
                    // fields are exactly what we just configured (PRG mount or empty).
                    // Clearing them would destroy the shortcut entry and file handle.
                    vdir.source.rewind();
                }
            }
            drop(disk_opt);

            self.handle_track_event(half_track).await;
        }
    }

    async fn refresh_paged_directory(&mut self, page: usize) {
        let (current_path, current_use_internal) = {
            let guard = self.mounted_dir.lock().await;
            (guard.0.clone(), guard.1)
        };
        let p_str = current_path.as_str();

        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                if let Some(paged) = vdir.source.as_paged_mut() {
                    paged.set_page(page);
                }
            }
        }

        let start_entry = page * crate::storage::PAGE_CAPACITY;
        let mut count = 0;
        let mut has_more = false;
        let disk_image = &mut self.disk_image;
        let _ = self
            .opener
            .list_dir_range(
                p_str,
                current_use_internal,
                start_entry,
                crate::storage::PAGE_CAPACITY + 1,
                |entry| {
                    if count < crate::storage::PAGE_CAPACITY {
                        if let Some(disk) = disk_image.as_mut() {
                            if let Some(vdir) = disk.as_virtual_disk_mut() {
                                if let Some(paged) = vdir.source.as_paged_mut() {
                                    paged.set_page_entry(count, entry);
                                }
                            }
                        }
                    } else {
                        has_more = true;
                    }
                    count += 1;
                },
            )
            .await;

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Page refresh: {} entries for \"{}\", page {}",
            count.min(crate::storage::PAGE_CAPACITY),
            p_str,
            page
        );

        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                if let Some(paged) = vdir.source.as_paged_mut() {
                    let loaded_count = count.min(crate::storage::PAGE_CAPACITY);
                    for i in loaded_count..crate::storage::PAGE_CAPACITY {
                        paged.set_page_entry(i, crate::storage::DirectoryEntry::default());
                    }
                    if count < crate::storage::PAGE_CAPACITY
                        || (count == crate::storage::PAGE_CAPACITY && !has_more)
                    {
                        paged.set_total_count(start_entry + count);
                    } else if has_more {
                        let known_min = (page + 1) * crate::storage::PAGE_CAPACITY + 1;
                        if paged.total_count() == usize::MAX || paged.total_count() < known_min {
                            paged.set_total_count(known_min);
                        }
                    }
                }
            }
        }

        self.track_handle.set_active_layout(TrackLayout::Void);
        self.track_handle.set_write_protected(true); // LOW (sliding out)
        embassy_time::Timer::after_millis(50).await;
        self.track_handle.set_write_protected(false); // HIGH (no disk)
        embassy_time::Timer::after_millis(400).await;
        self.track_handle.set_write_protected(true); // LOW (sliding in)
        embassy_time::Timer::after_millis(50).await;
        self.track_handle.set_write_protected(true);

        self.track_handle.force_disk_change();

        let half_track = self.track_handle.get_head_half_track();
        if let Some(disk) = self.disk_image.as_mut() {
            if let Some(vdir) = disk.as_virtual_disk_mut() {
                vdir.source.rewind();
            }
        }
        self.handle_track_event(half_track).await;
    }

    pub async fn select_virtual_file(&mut self, slot: usize) {
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
                    if !entry.is_dir() && entry.filename[0] != 0 {
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


pub static CURRENT_PAGE: portable_atomic::AtomicUsize = portable_atomic::AtomicUsize::new(0);
pub static PAGE_CHANGED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
pub static SELECTED_SLOT: portable_atomic::AtomicUsize = portable_atomic::AtomicUsize::new(usize::MAX);
pub static SLOT_SELECTED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);

pub struct MountControllerTask<L: LedsDriver> {
    leds: L,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    motor_state: &'static AtomicBool,
    cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
    current_page: usize,
}

impl<L: LedsDriver> MountControllerTask<L> {
    pub fn new(
        leds: L,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        motor_state: &'static AtomicBool,
        cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
    ) -> Self {
        Self {
            leds,
            mounted_dir,
            motor_state,
            cmd_rx,
            current_page: 0,
        }
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub async fn run(&mut self) {
        loop {
            let cmd = self.cmd_rx.receive().await;
            match cmd {
                crate::InternalCommand::MotorState(active) => {
                    self.motor_state.store(active, Ordering::Relaxed);
                }
                crate::InternalCommand::Mount(payload) => {
                    self.current_page = 0;
                    CURRENT_PAGE.store(0, Ordering::Release);
                    let (path, use_internal, led_mode) = payload.into_parts();

                    {
                        let mut guard = self.mounted_dir.lock().await;
                        guard.0 = path;
                        guard.1 = use_internal;
                    }
                    self.leds.set_status(led_mode);
                }
                crate::InternalCommand::Unmount => {
                    self.current_page = 0;
                    CURRENT_PAGE.store(0, Ordering::Release);
                    {
                        let mut guard = self.mounted_dir.lock().await;
                        guard.0 = heapless::String::new();
                        guard.1 = false;
                    }
                    self.leds
                        .set_status(crate::driver::StatusLedMode::Unmounted);
                }
                crate::InternalCommand::Cd(cd_str) => {
                    let current = {
                        let guard = self.mounted_dir.lock().await;
                        guard.0.clone()
                    };
                    if let Some(action) =
                        cd_handler::resolve_cd_action(current.as_str(), cd_str.as_str())
                    {
                        match action {
                            cd_handler::CdAction::NextPage => {
                                self.current_page += 1;
                                CURRENT_PAGE.store(self.current_page, Ordering::Release);
                                PAGE_CHANGED.store(true, Ordering::Release);
                            }
                            cd_handler::CdAction::PrevPage => {
                                self.current_page = self.current_page.saturating_sub(1);
                                CURRENT_PAGE.store(self.current_page, Ordering::Release);
                                PAGE_CHANGED.store(true, Ordering::Release);
                            }
                            cd_handler::CdAction::ChangeDir(new_path) => {
                                self.current_page = 0;
                                CURRENT_PAGE.store(0, Ordering::Release);
                                {
                                    let mut guard = self.mounted_dir.lock().await;
                                    guard.0 = new_path;
                                    guard.1 = false; // always SD card for SD2IEC nav
                                }
                                self.leds.set_status(crate::driver::StatusLedMode::VirtualDisk);
                            }
                            cd_handler::CdAction::MountImage(new_path) => {
                                self.current_page = 0;
                                CURRENT_PAGE.store(0, Ordering::Release);
                                {
                                    let mut guard = self.mounted_dir.lock().await;
                                    guard.0 = new_path;
                                    guard.1 = false; // always SD card for SD2IEC nav
                                }
                                self.leds.set_status(crate::driver::StatusLedMode::NonAccurateDisk);
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
