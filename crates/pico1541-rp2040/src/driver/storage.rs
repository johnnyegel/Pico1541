use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager, VolumeIdx, Mode, ShortFileName};
use pico1541_manager::driver::{StorageDriver, FileStorageDriver, FileInfo};
use pico1541_common::formats::Error as ManagerError;
use pico1541_common::storage::Error as CommonError;
use embedded_io_async::{Read, Seek, SeekFrom, ErrorType};
use heapless::String;

pub struct DummyTimeSource;

impl TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 56, // 2026
            zero_indexed_month: 4, // May
            zero_indexed_day: 12,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub struct SdStorage<SPI, CS, D, DET> 
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs,
    DET: embedded_hal::digital::InputPin
{
    pub volume_mgr: VolumeManager<SdCard<SPI, CS, D>, DummyTimeSource>,
    pub det_pin: DET,
}

pub struct StorageWrapper<SPI, CS, D, DET>(
    pub &'static embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, SdStorage<SPI, CS, D, DET>>,
)
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static;

impl<SPI, CS, D, DET> Clone for StorageWrapper<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<SPI, CS, D, DET> Copy for StorageWrapper<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{}

pub struct SdFileHandle<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    storage: StorageWrapper<SPI, CS, D, DET>,
    path: String<32>,
    offset: u64,
}

impl<SPI, CS, D, DET> ErrorType for SdFileHandle<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    type Error = CommonError;
}

impl<SPI, CS, D, DET> Read for SdFileHandle<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut storage = self.storage.0.lock().await;
        if storage.det_pin.is_high().map_err(|_| CommonError::Io)? {
            return Err(CommonError::Io);
        }
        let mut volume = storage.volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| CommonError::Io)?;
        let mut root = volume.open_root_dir().map_err(|_| CommonError::Io)?;
        let filename = ShortFileName::create_from_str(&self.path).map_err(|_| CommonError::Io)?;
        let mut file = root.open_file_in_dir(filename, Mode::ReadOnly).map_err(|_| CommonError::Io)?;
        
        file.seek_from_start(self.offset as u32).map_err(|_| CommonError::Io)?;
        let n = file.read(buf).map_err(|_| CommonError::Io)?;
        self.offset = file.offset() as u64;
        Ok(n)
    }
}

impl<SPI, CS, D, DET> Seek for SdFileHandle<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Start(n) => self.offset = n,
            SeekFrom::Current(n) => self.offset = (self.offset as i64 + n) as u64,
            SeekFrom::End(n) => {
                let mut storage = self.storage.0.lock().await;
                if storage.det_pin.is_high().map_err(|_| CommonError::Io)? {
                    return Err(CommonError::Io);
                }
                let mut volume = storage.volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| CommonError::Io)?;
                let mut root = volume.open_root_dir().map_err(|_| CommonError::Io)?;
                let filename = ShortFileName::create_from_str(&self.path).map_err(|_| CommonError::Io)?;
                let file = root.open_file_in_dir(filename, Mode::ReadOnly).map_err(|_| CommonError::Io)?;
                self.offset = (file.length() as i64 + n) as u64;
            }
        }
        Ok(self.offset)
    }
}

impl<SPI, CS, D, DET> FileStorageDriver for StorageWrapper<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    async fn is_available(&self) -> bool {
        if let Ok(mut storage) = self.0.try_lock() {
            storage.det_pin.is_low().unwrap_or(false)
        } else {
            false
        }
    }

    async fn list_dir(&mut self, _path: &str, mut callback: impl FnMut(FileInfo)) -> Result<(), ManagerError> {
        let mut storage = self.0.lock().await;
        if storage.det_pin.is_high().map_err(|_| ManagerError::Io)? {
            return Err(ManagerError::Io);
        }
        let mut volume = match storage.volume_mgr.open_volume(VolumeIdx(0)) {
            Ok(v) => v,
            Err(_) => return Err(ManagerError::Io),
        };
        let mut root = match volume.open_root_dir() {
            Ok(r) => r,
            Err(_) => return Err(ManagerError::Io),
        };
        let _ = root.iterate_dir(|entry| {
            let mut info = FileInfo {
                name: [0; 13],
                size: entry.size,
                is_dir: entry.attributes.is_directory(),
            };
            let name_bytes = entry.name.base_name();
            let len = name_bytes.len().min(8);
            info.name[..len].copy_from_slice(name_bytes);
            callback(info);
        });
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), ManagerError> {
        Ok(())
    }
}

impl<SPI, CS, D, DET> StorageWrapper<SPI, CS, D, DET>
where
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
}
impl<SPI, CS, D, DET> SdStorage<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs,
    DET: embedded_hal::digital::InputPin
{
    pub fn new(spi_device: SPI, cs_pin: CS, delayer: D, det_pin: DET) -> Self {
        let sdcard = SdCard::new(spi_device, cs_pin, delayer);
        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        Self { volume_mgr, det_pin }
    }
}

impl<SPI, CS, D, DET> StorageDriver for SdStorage<SPI, CS, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs,
    DET: embedded_hal::digital::InputPin
{
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), ManagerError> {
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), ManagerError> {
        Ok(())
    }
}
