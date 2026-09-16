use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::Mutex;
use embedded_hal::digital::InputPin;

use crate::driver::file_storage::FatStorage;

use crate::driver::ResettableBlockDevice;

pub struct SdMonitorTask<M, B, Det>
where
    M: RawMutex + 'static,
    B: fatfs::Read
        + fatfs::Write
        + fatfs::Seek
        + ResettableBlockDevice
        + Send
        + Sync
        + Clone
        + 'static,
    Det: InputPin + Send + 'static,
{
    sd_mutex: &'static Mutex<M, FatStorage<B, Det>>,
}

impl<M, B, Det> SdMonitorTask<M, B, Det>
where
    M: RawMutex + 'static,
    B: fatfs::Read
        + fatfs::Write
        + fatfs::Seek
        + ResettableBlockDevice
        + Send
        + Sync
        + Clone
        + 'static,
    Det: InputPin + Send + 'static,
{
    pub fn new(sd_mutex: &'static Mutex<M, FatStorage<B, Det>>) -> Self {
        Self { sd_mutex }
    }

    pub async fn run(&mut self) {
        loop {
            let mut mounted = false;
            {
                let mut storage = self.sd_mutex.lock().await;
                if storage.det_pin.is_low().unwrap_or(false) && storage.fat_fs.is_none() {
                    if storage.ensure_mounted().await.is_ok() {
                        mounted = true;
                    }
                } else if storage.det_pin.is_high().unwrap_or(false) {
                    storage.fat_fs = None;
                    storage.card_init = false;
                    storage.block_dev.reset();
                }
            }
            if !mounted {
                embassy_time::Timer::after(embassy_time::Duration::from_millis(50)).await;
            } else {
                embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
            }
        }
    }
}
