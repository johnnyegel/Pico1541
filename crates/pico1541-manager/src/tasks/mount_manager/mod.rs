use crate::formats::AnyDiskImage;
use core::sync::atomic::Ordering;
use embassy_futures::select::{select3, Either3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::ManagerHandle;

mod cmd;
mod file_opener;
mod timer;
mod track;

pub use file_opener::{match_c64_filename, FileOpener};

pub struct MountManagerTask<
    S: Read + Seek + 'static,
    O: FileOpener<FileStorage = S>,
    L: crate::driver::RgbLedDriver,
> {
    pub(super) disk_image: Option<AnyDiskImage<S, crate::storage::PagedDirectorySource<144>>>,
    pub(super) track_handle: ManagerHandle,
    pub(super) opener: O,
    pub(super) leds: L,
    pub(super) mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    pub(super) pre_cached_ht: Option<(
        u8,
        pico1541_common::track_buffer::TrackLayout,
        usize,
        Option<u8>,
    )>,
    pub(super) last_step_direction: i8,
}

impl<S: Read + Seek + 'static, O: FileOpener<FileStorage = S>, L: crate::driver::RgbLedDriver>
    MountManagerTask<S, O, L>
{
    pub fn new(
        track_handle: ManagerHandle,
        opener: O,
        leds: L,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    ) -> Self {
        Self {
            disk_image: None,
            track_handle,
            opener,
            leds,
            mounted_dir,
            pre_cached_ht: None,
            last_step_direction: 2,
        }
    }

    pub async fn run(&mut self) {
        let mut motor_on = false;
        loop {
            // Process all pending UI commands first to avoid starvation from runaway stepper motors
            while let Ok(cmd) = crate::CMD_CHANNEL.try_receive() {
                self.handle_cmd_event(cmd, &mut motor_on).await;
            }

            if crate::TRACK_SIGNAL.signaled() {
                let half_track = crate::TRACK_REQUEST.load(Ordering::Relaxed);
                crate::TRACK_SIGNAL.reset();
                self.handle_track_event(half_track).await;
                continue;
            }

            let track_fut = crate::TRACK_SIGNAL.wait();
            let timer_fut = embassy_time::Timer::after_millis(20);
            let cmd_fut = crate::CMD_CHANNEL.receive();

            match select3(track_fut, timer_fut, cmd_fut).await {
                Either3::First(_) => {
                    self.handle_track_event(crate::TRACK_REQUEST.load(Ordering::Relaxed))
                        .await;
                }
                Either3::Second(_) => {
                    self.handle_timer_event(&mut motor_on).await;
                }
                Either3::Third(cmd) => {
                    self.handle_cmd_event(cmd, &mut motor_on).await;
                }
            }
        }
    }
}
