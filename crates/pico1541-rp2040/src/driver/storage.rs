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

pub struct SdStorage<SPI, CS, D> 
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs
{
    pub volume_mgr: VolumeManager<SdCard<SPI, CS, D>, DummyTimeSource>,
}

pub struct StorageWrapper<SPI, CS, D>(
    pub &'static embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, SdStorage<SPI, CS, D>>,
)
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static;

impl<SPI, CS, D> Clone for StorageWrapper<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<SPI, CS, D> Copy for StorageWrapper<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{}

pub struct SdFileHandle<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    storage: StorageWrapper<SPI, CS, D>,
    path: String<32>,
    offset: u64,
}

impl<SPI, CS, D> ErrorType for SdFileHandle<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    type Error = CommonError;
}

impl<SPI, CS, D> Read for SdFileHandle<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut storage = self.storage.0.lock().await;
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

impl<SPI, CS, D> Seek for SdFileHandle<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Start(n) => self.offset = n,
            SeekFrom::Current(n) => self.offset = (self.offset as i64 + n) as u64,
            SeekFrom::End(n) => {
                let mut storage = self.storage.0.lock().await;
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

impl<SPI, CS, D> FileStorageDriver for StorageWrapper<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    async fn list_dir(&mut self, _path: &str, mut callback: impl FnMut(FileInfo)) -> Result<(), ManagerError> {
        let mut storage = self.0.lock().await;
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

impl<SPI, CS, D> StorageWrapper<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    D: embedded_hal::delay::DelayNs + 'static
{
    pub async fn open_sd_file(&self, path: &str) -> Result<SdFileHandle<SPI, CS, D>, ManagerError> {
        let mut p = String::<32>::new();
        p.push_str(path).map_err(|_| ManagerError::Io)?;
        
        Ok(SdFileHandle {
            storage: *self,
            path: p,
            offset: 0,
        })
    }
}

impl<SPI, CS, D> SdStorage<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs
{
    pub fn new(spi_device: SPI, cs_pin: CS, delayer: D) -> Self {
        let sdcard = SdCard::new(spi_device, cs_pin, delayer);
        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        Self { volume_mgr }
    }
}

impl<SPI, CS, D> StorageDriver for SdStorage<SPI, CS, D>
where 
    SPI: embedded_hal::spi::SpiDevice,
    CS: embedded_hal::digital::OutputPin,
    D: embedded_hal::delay::DelayNs
{
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), ManagerError> {
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), ManagerError> {
        Ok(())
    }
}
