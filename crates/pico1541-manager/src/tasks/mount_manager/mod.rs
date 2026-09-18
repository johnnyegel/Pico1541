use crate::driver::RgbLedDriver as LedsDriver;
use crate::formats::AnyDiskImage;
use crate::formats::DiskImage;
use crate::storage::DirectorySource;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_io_async::{Read, Seek};
use pico1541_common::interface::TrackHandoff;
use pico1541_common::track_buffer::{ManagerHandle, TrackLayout};

mod timer;
mod track;

pub use file_opener::{match_c64_filename, FileOpener};
pub mod file_opener;

pub struct TrackCacheTask<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, H: TrackHandoff>
{
    pub(super) disk_image: Option<AnyDiskImage<S, crate::storage::PagedDirectorySource<144>>>,
    pub(super) track_handle: ManagerHandle,
    pub(super) opener: O,
    pub(super) handoff: H,
    pub(super) mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    pub(super) pre_cached_ht: Option<(
        u8,
        pico1541_common::track_buffer::TrackLayout,
        usize,
        Option<u8>,
    )>,
    pub(super) last_step_direction: i8,
    last_mounted_path: heapless::String<128>,
    last_motor_state: bool,
    motor_state: &'static AtomicBool,
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
            pre_cached_ht: None,
            last_step_direction: 2,
            last_mounted_path: heapless::String::new(),
            last_motor_state: false,
            motor_state,
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
                        vdir.next_sector_to_update = 1;
                        let data = self.track_handle.start_track_modification();
                        vdir.force_update_directory_sectors(data).await;
                        self.track_handle.commit_modified_track();
                        vdir.treadmill_spinup_delay = 10;
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

    async fn mount_sequence(&mut self, path: heapless::String<128>, use_internal: bool) {
        let p_str = path.as_str();

        let is_d64 = p_str.ends_with(".d64") || p_str.ends_with(".D64");
        let is_g64 = p_str.ends_with(".g64") || p_str.ends_with(".G64");

        let ht1 = self.track_handle.get_head_half_track();
        let mut ht2 = ht1;

        if is_d64 {
            let file = if use_internal {
                self.opener.open_internal_file(p_str)
            } else {
                self.opener.open_sd_file(p_str)
            };
            let d64 = crate::formats::D64Image::new(file).await;
            self.disk_image = Some(crate::formats::AnyDiskImage::D64(d64));
        } else if is_g64 {
            let file = if use_internal {
                self.opener.open_internal_file(p_str)
            } else {
                self.opener.open_sd_file(p_str)
            };
            if let Ok(g64) = crate::formats::G64Image::from_storage(file).await {
                self.disk_image = Some(crate::formats::AnyDiskImage::G64(g64));
            } else {
                self.disk_image = None;
            }
            ht2 = self.track_handle.get_head_half_track();
        } else {
            let dir_source = crate::storage::PagedDirectorySource::<144>::new(path.as_bytes());
            let vdisk = crate::formats::VirtualDiskImage::new(dir_source, path.as_bytes());
            self.disk_image = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
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
                    vdir.set_shortcut_entry(None);
                    vdir.file_storage = None;
                    vdir.source.rewind();
                    if let Some(paged) = vdir.source.as_paged_mut() {
                        paged.invalidate();
                    }
                }
            }
            drop(disk_opt);

            let mut disk_opt = self.disk_image.as_mut();
            if let Some(disk) = disk_opt.as_mut() {
                let data = self.track_handle.start_track_load();
                match disk.read_track(half_track, data).await {
                    Ok(layout) => {
                        self.track_handle.commit_loaded_track(
                            pico1541_common::track_buffer::TrackIdentifier::new(
                                half_track,
                                layout.2.unwrap_or_else(|| {
                                    pico1541_common::track_buffer::default_speed_zone(half_track)
                                }),
                            ),
                            layout.0,
                            layout.1,
                        );
                    }
                    Err(_) => {
                        self.track_handle.set_active_layout(TrackLayout::Void);
                    }
                }
            }
        }
    }
}

use embassy_sync::channel::Receiver;

pub struct MountControllerTask<L: LedsDriver> {
    leds: L,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    motor_state: &'static AtomicBool,
    cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
}

impl<L: LedsDriver> MountControllerTask<L> {
    pub fn new(
        leds: L,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        motor_state: &'static AtomicBool,
        cmd_rx: Receiver<'static, CriticalSectionRawMutex, crate::InternalCommand, 128>,
    ) -> Self {
        Self { leds, mounted_dir, motor_state, cmd_rx }
    }

    pub async fn run(&mut self) {
        loop {
            let cmd = self.cmd_rx.receive().await;
            match cmd {
                crate::InternalCommand::MotorState(active) => {
                    self.motor_state.store(active, Ordering::Relaxed);
                }
                crate::InternalCommand::Mount(payload) => {
                    let (path, use_internal, led_mode) = match payload {
                        crate::MountPayload::VirtualDirectory(p, i) => {
                            (p, i, crate::driver::StatusLedMode::VirtualDisk)
                        }
                        crate::MountPayload::D64(p, i) => {
                            (p, i, crate::driver::StatusLedMode::NonAccurateDisk)
                        }
                        crate::MountPayload::G64(p, i) => {
                            (p, i, crate::driver::StatusLedMode::AccurateDisk)
                        }
                    };

                    {
                        let mut guard = self.mounted_dir.lock().await;
                        guard.0 = path;
                        guard.1 = use_internal;
                    }
                    self.leds.set_status(led_mode);
                }
                crate::InternalCommand::Unmount => {
                    {
                        let mut guard = self.mounted_dir.lock().await;
                        guard.0 = heapless::String::new();
                        guard.1 = false;
                    }
                    self.leds
                        .set_status(crate::driver::StatusLedMode::Unmounted);
                }
            }
        }
    }
}
