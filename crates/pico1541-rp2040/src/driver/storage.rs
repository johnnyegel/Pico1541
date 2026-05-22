use pico1541_manager::driver::{StorageDriver, FileStorageDriver, FileInfo};
use pico1541_common::formats::Error as ManagerError;
use pico1541_common::storage::Error as CommonError;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use sdmmc::SD;
use sdmmc::CSD;
use hadris_fat::r#async::fs::FatFs;
use hadris_fat::r#async::dir::DirectoryEntry;
use hadris_fat::DirEntryAttrFlags;
use hadris_io::SeekFrom;

use embedded_timers::clock::Clock;
use embedded_timers::instant::Instant64;

pub struct EmbassyClock;

impl Clock for EmbassyClock {
    type Instant = Instant64<1_000_000>;

    fn now(&self) -> Self::Instant {
        Instant64::new(embassy_time::Instant::now().as_micros())
    }
}

pub struct EmbassyDelay;

impl sdmmc::delay::Delay for EmbassyDelay {
    type Future = embassy_time::Timer;

    fn delay_ms(&mut self, ms: u32) -> Self::Future {
        embassy_time::Timer::after_millis(ms as u64)
    }
}

pub trait SdSpiBus {
    async fn init_bus(&mut self, delay: EmbassyDelay) -> Result<sdmmc::Card, ManagerError>;
    fn set_high_speed(&mut self);
}

impl<T, CS, C, I> SdSpiBus for sdmmc::bus::spi::Bus<embassy_rp::spi::Spi<'static, T, embassy_rp::spi::Async>, CS, C>
where
    T: embassy_rp::spi::Instance + 'static,
    CS: embedded_hal::digital::OutputPin + 'static,
    C: embedded_timers::clock::Clock<Instant = I> + 'static,
    I: embedded_timers::instant::Instant + 'static,
{
    async fn init_bus(&mut self, delay: EmbassyDelay) -> Result<sdmmc::Card, ManagerError> {
        self.init(delay).await.map_err(|_| ManagerError::Io)
    }

    fn set_high_speed(&mut self) {
        self.spi(|spi| {
            spi.set_frequency(4_000_000);
        });
    }
}

pub struct RefBus<BUS> {
    bus_ptr: *mut BUS,
}

unsafe impl<BUS> Send for RefBus<BUS> {}
unsafe impl<BUS> Sync for RefBus<BUS> {}

impl<BUS> sdmmc::bus::Bus for RefBus<BUS>
where
    BUS: sdmmc::bus::Bus,
{
    type Error = BUS::Error;

    fn before(&mut self) -> Result<(), sdmmc::bus::Error<Self::Error>> {
        unsafe { &mut *self.bus_ptr }.before()
    }

    fn after(&mut self) -> Result<(), sdmmc::bus::Error<Self::Error>> {
        unsafe { &mut *self.bus_ptr }.after()
    }
}

impl<BUS> sdmmc::bus::Read for RefBus<BUS>
where
    BUS: sdmmc::bus::Read + sdmmc::bus::Bus<Error = <BUS as sdmmc::bus::Read>::Error> + 'static,
{
    type Error = <BUS as sdmmc::bus::Bus>::Error;

    async fn read_csd(&mut self) -> Result<CSD, sdmmc::bus::Error<Self::Error>> {
        unsafe { &mut *self.bus_ptr }.read_csd().await
    }

    async fn read<'a, B>(&mut self, block: u32, blocks: B) -> Result<(), sdmmc::bus::Error<Self::Error>>
    where
        B: core::iter::ExactSizeIterator<Item = &'a mut [u8; 512]>,
    {
        unsafe { &mut *self.bus_ptr }.read(block, blocks).await
    }
}

impl<BUS> sdmmc::bus::Write for RefBus<BUS>
where
    BUS: sdmmc::bus::Write + sdmmc::bus::Bus<Error = <BUS as sdmmc::bus::Write>::Error> + 'static,
{
    type Error = <BUS as sdmmc::bus::Bus>::Error;

    async fn write<'a, B>(&mut self, block: u32, blocks: B) -> Result<(), sdmmc::bus::Error<Self::Error>>
    where
        B: core::iter::ExactSizeIterator<Item = &'a [u8; 512]>,
    {
        unsafe { &mut *self.bus_ptr }.write(block, blocks).await
    }
}

pub struct SdBlockIo<BUS> {
    sd_ptr: *mut SD<RefBus<BUS>>,
    offset: u64,
    cache: [u8; 512],
    cached_sector: Option<u32>,
}

unsafe impl<BUS> Send for SdBlockIo<BUS> {}
unsafe impl<BUS> Sync for SdBlockIo<BUS> {}

impl<BUS> SdBlockIo<BUS> {
    pub fn new(sd_ptr: *mut SD<RefBus<BUS>>) -> Self {
        Self {
            sd_ptr,
            offset: 0,
            cache: [0u8; 512],
            cached_sector: None,
        }
    }
}

impl<BUS> hadris_io::r#async::Read for SdBlockIo<BUS>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, hadris_io::Error> {
        let mut total_read = 0;
        let len = buf.len();
        let sd = unsafe { &mut *self.sd_ptr };
        
        while total_read < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_read).min(bytes_avail);
            
            if self.cached_sector != Some(sector) {
                let mut sector_data = [0u8; 512];
                let blocks = core::iter::once(&mut sector_data);
                sd.read(sector, blocks).await.map_err(|_| {
                    hadris_io::Error::new_static(hadris_io::ErrorKind::Other, "SD read error")
                })?;
                self.cache = sector_data;
                self.cached_sector = Some(sector);
            }
            
            let src = &self.cache[offset_in_sector..offset_in_sector + to_copy];
            buf[total_read..total_read + to_copy].copy_from_slice(src);
            
            self.offset += to_copy as u64;
            total_read += to_copy;
        }
        
        Ok(total_read)
    }
}

impl<BUS> hadris_io::r#async::Seek for SdBlockIo<BUS>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + 'static,
{
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, hadris_io::Error> {
        let sd = unsafe { &mut *self.sd_ptr };
        let new_offset = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(n) => (self.offset as i64 + n) as u64,
            SeekFrom::End(n) => {
                let num_blocks: u64 = sd.num_blocks().into();
                let total_bytes = num_blocks * 512;
                (total_bytes as i64 + n) as u64
            }
        };
        self.offset = new_offset;
        Ok(new_offset)
    }
}

impl<BUS> hadris_io::r#async::Write for SdBlockIo<BUS>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, hadris_io::Error> {
        let mut total_written = 0;
        let len = buf.len();
        let sd = unsafe { &mut *self.sd_ptr };
        
        while total_written < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_written).min(bytes_avail);

            if self.cached_sector != Some(sector) {
                let mut sector_data = [0u8; 512];
                let blocks = core::iter::once(&mut sector_data);
                if sd.read(sector, blocks).await.is_ok() {
                    self.cache = sector_data;
                } else {
                    self.cache = [0u8; 512];
                }
                self.cached_sector = Some(sector);
            }

            self.cache[offset_in_sector..offset_in_sector + to_copy].copy_from_slice(&buf[total_written..total_written + to_copy]);
            
            let blocks = core::iter::once(&self.cache);
            sd.write(sector, blocks).await.map_err(|_| {
                hadris_io::Error::new_static(hadris_io::ErrorKind::Other, "SD write error")
            })?;

            self.offset += to_copy as u64;
            total_written += to_copy;
        }
        Ok(total_written)
    }

    async fn flush(&mut self) -> Result<(), hadris_io::Error> {
        Ok(())
    }
}

pub struct SdStorage<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    pub bus: BUS,
    pub sd: Option<SD<RefBus<BUS>>>,
    pub fat_fs: Option<FatFs<SdBlockIo<BUS>>>,
    pub det_pin: DET,
    pub volume_label: Option<heapless::String<11>>,
}

impl<BUS, DET> SdStorage<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    pub fn new(bus: BUS, det_pin: DET) -> Self {
        Self {
            bus,
            sd: None,
            fat_fs: None,
            det_pin,
            volume_label: None,
        }
    }

    pub async fn ensure_mounted(&mut self) -> Result<&FatFs<SdBlockIo<BUS>>, ManagerError> {
        if self.det_pin.is_high().map_err(|_| ManagerError::Io)? {
            self.fat_fs = None;
            self.sd = None;
            return Err(ManagerError::Io);
        }

        if self.fat_fs.is_none() {
            defmt::info!("SD: initiating card bus...");
            let card = match self.bus.init_bus(EmbassyDelay).await {
                Ok(c) => c,
                Err(e) => {
                    defmt::error!("SD: init_bus failed: {:?}", defmt::Debug2Format(&e));
                    return Err(e);
                }
            };
            defmt::info!("SD: init_bus success. Setting high speed...");
            self.bus.set_high_speed();
            
            let ref_bus = RefBus { bus_ptr: &mut self.bus as *mut BUS };
            defmt::info!("SD: initializing SD reader...");
            let sd = match SD::init(ref_bus, card).await {
                Ok(s) => s,
                Err(e) => {
                    match e {
                        sdmmc::bus::Error::NoResponse => defmt::error!("SD: SD::init failed: NoResponse"),
                        sdmmc::bus::Error::NotIdle => defmt::error!("SD: SD::init failed: NotIdle"),
                        sdmmc::bus::Error::Command(status) => defmt::error!("SD: SD::init failed: Command({:?})", defmt::Debug2Format(&status)),
                        sdmmc::bus::Error::Transfer(token) => defmt::error!("SD: SD::init failed: Transfer({:?})", defmt::Debug2Format(&token)),
                        sdmmc::bus::Error::Timeout => defmt::error!("SD: SD::init failed: Timeout"),
                        sdmmc::bus::Error::Generic => defmt::error!("SD: SD::init failed: Generic"),
                        _ => defmt::error!("SD: SD::init failed: Bus error or unknown"),
                    }
                    return Err(ManagerError::Io);
                }
            };
            self.sd = Some(sd);

            let block_io = SdBlockIo::new(self.sd.as_mut().unwrap() as *mut SD<RefBus<BUS>>);
            defmt::info!("SD: opening FatFs...");
            let fs = match FatFs::open(block_io).await {
                Ok(f) => f,
                Err(e) => {
                    defmt::error!("SD: FatFs::open failed: {:?}", defmt::Debug2Format(&e));
                    return Err(ManagerError::Io);
                }
            };
            self.fat_fs = Some(fs);
            defmt::info!("SD: mounted successfully!");
        }

        Ok(self.fat_fs.as_ref().unwrap())
    }
}

pub struct StorageWrapper<BUS, DET>(
    pub &'static Mutex<CriticalSectionRawMutex, SdStorage<BUS, DET>>,
)
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static;

impl<BUS, DET> Clone for StorageWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<BUS, DET> Copy for StorageWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{}

pub struct HadrisFileWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    storage: StorageWrapper<BUS, DET>,
    path: heapless::String<128>,
    offset: u64,
}

impl<BUS, DET> embedded_io_async::ErrorType for HadrisFileWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    type Error = CommonError;
}

impl<BUS, DET> embedded_io_async::Read for HadrisFileWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut storage = self.storage.0.lock().await;
        let fs = storage.ensure_mounted().await.map_err(|_| CommonError::Io)?;
        
        let mut file_reader = fs.open_file_path(self.path.as_str()).await.map_err(|_| CommonError::Io)?;
        
        let to_discard = self.offset as usize;
        let mut discarded = 0;
        let mut discard_buf = [0u8; 256];
        while discarded < to_discard {
            let chunk = (to_discard - discarded).min(discard_buf.len());
            let n = file_reader.read(&mut discard_buf[..chunk]).await.map_err(|_| CommonError::Io)?;
            if n == 0 {
                break;
            }
            discarded += n;
        }
        
        let read_len = file_reader.read(buf).await.map_err(|_| CommonError::Io)?;
        self.offset += read_len as u64;
        Ok(read_len)
    }
}

impl<BUS, DET> embedded_io_async::Seek for HadrisFileWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    async fn seek(&mut self, pos: embedded_io_async::SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            embedded_io_async::SeekFrom::Start(n) => self.offset = n,
            embedded_io_async::SeekFrom::Current(n) => self.offset = (self.offset as i64 + n) as u64,
            embedded_io_async::SeekFrom::End(n) => {
                let mut storage = self.storage.0.lock().await;
                let fs = storage.ensure_mounted().await.map_err(|_| CommonError::Io)?;
                let file_reader = fs.open_file_path(self.path.as_str()).await.map_err(|_| CommonError::Io)?;
                let len = file_reader.size() as i64;
                self.offset = (len + n) as u64;
            }
        }
        Ok(self.offset)
    }
}

impl<BUS, DET> FileStorageDriver for StorageWrapper<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    type File = HadrisFileWrapper<BUS, DET>;

    async fn is_available(&self) -> bool {
        if let Ok(mut storage) = self.0.try_lock() {
            storage.det_pin.is_low().map_err(|_| ()).unwrap_or(false)
        } else {
            false
        }
    }

    async fn list_dir(&mut self, path: &str, mut callback: impl FnMut(FileInfo)) -> Result<(), ManagerError> {
        let mut storage = self.0.lock().await;
        
        if path == "/" {
            storage.volume_label = None;
        }

        let mut temp_volume_label = None;
        {
            let fs = storage.ensure_mounted().await?;
            let dir = fs.open_dir_path(path).await.map_err(|_| ManagerError::Io)?;
            let mut iter = dir.entries();
            
            while let Some(entry_res) = iter.next_entry().await {
                let entry = entry_res.map_err(|_| ManagerError::Io)?;
                let DirectoryEntry::Entry(file_entry) = entry;
                
                if file_entry.attributes().contains(DirEntryAttrFlags::VOLUME_ID) {
                    let name = file_entry.name();
                    let mut label = heapless::String::new();
                    let _ = label.push_str(name.trim());
                    temp_volume_label = Some(label);
                    continue;
                }

                let name = file_entry.name();
                if name.starts_with('.') {
                    continue;
                }

                let mut info = FileInfo {
                    name: [0; 64],
                    size: file_entry.size() as u32,
                    is_dir: file_entry.is_directory(),
                };
                let mut writer = WrapperWriter::new(&mut info.name);
                let _ = core::fmt::write(&mut writer, format_args!("{}", name));
                callback(info);
            }
        }
        
        if temp_volume_label.is_some() {
            storage.volume_label = temp_volume_label;
        }
        
        Ok(())
    }

    async fn count_dir_entries(&mut self, path: &str) -> Result<usize, ManagerError> {
        let mut storage = self.0.lock().await;
        
        if path == "/" {
            storage.volume_label = None;
        }

        let mut count = 0;
        let mut temp_volume_label = None;
        {
            let fs = storage.ensure_mounted().await?;
            let dir = fs.open_dir_path(path).await.map_err(|_| ManagerError::Io)?;
            let mut iter = dir.entries();
            
            while let Some(entry_res) = iter.next_entry().await {
                let entry = entry_res.map_err(|_| ManagerError::Io)?;
                let DirectoryEntry::Entry(file_entry) = entry;
                
                if file_entry.attributes().contains(DirEntryAttrFlags::VOLUME_ID) {
                    let name = file_entry.name();
                    let mut label = heapless::String::new();
                    let _ = label.push_str(name.trim());
                    temp_volume_label = Some(label);
                    continue;
                }

                let name = file_entry.name();
                if name.starts_with('.') {
                    continue;
                }

                count += 1;
            }
        }
        
        if temp_volume_label.is_some() {
            storage.volume_label = temp_volume_label;
        }
        
        Ok(count)
    }

    async fn list_dir_range(
        &mut self,
        path: &str,
        start_index: usize,
        count: usize,
        mut callback: impl FnMut(FileInfo),
    ) -> Result<(), ManagerError> {
        let mut storage = self.0.lock().await;
        
        if path == "/" {
            storage.volume_label = None;
        }

        let mut current_idx = 0;
        let mut temp_volume_label = None;
        {
            let fs = storage.ensure_mounted().await?;
            let dir = fs.open_dir_path(path).await.map_err(|_| ManagerError::Io)?;
            let mut iter = dir.entries();
            
            while let Some(entry_res) = iter.next_entry().await {
                if current_idx >= start_index + count {
                    break;
                }

                let entry = entry_res.map_err(|_| ManagerError::Io)?;
                let DirectoryEntry::Entry(file_entry) = entry;
                
                if file_entry.attributes().contains(DirEntryAttrFlags::VOLUME_ID) {
                    let name = file_entry.name();
                    let mut label = heapless::String::new();
                    let _ = label.push_str(name.trim());
                    temp_volume_label = Some(label);
                    continue;
                }

                let name = file_entry.name();
                if name.starts_with('.') {
                    continue;
                }

                if current_idx >= start_index {
                    let mut info = FileInfo {
                        name: [0; 64],
                        size: file_entry.size() as u32,
                        is_dir: file_entry.is_directory(),
                    };
                    let mut writer = WrapperWriter::new(&mut info.name);
                    let _ = core::fmt::write(&mut writer, format_args!("{}", name));
                    callback(info);
                }
                
                current_idx += 1;
            }
        }
        
        if temp_volume_label.is_some() {
            storage.volume_label = temp_volume_label;
        }
        
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), ManagerError> {
        Ok(())
    }

    fn open_file_handle(&self, path: &str) -> Self::File {
        use core::str::FromStr;
        HadrisFileWrapper {
            storage: *self,
            path: heapless::String::from_str(path).unwrap_or_default(),
            offset: 0,
        }
    }

    fn volume_label(&self) -> Option<heapless::String<11>> {
        if let Ok(storage) = self.0.try_lock() {
            storage.volume_label.clone()
        } else {
            None
        }
    }
}

impl<BUS, DET> StorageDriver for SdStorage<BUS, DET>
where
    BUS: sdmmc::bus::Bus + sdmmc::bus::Read<Error = <BUS as sdmmc::bus::Bus>::Error> + sdmmc::bus::Write<Error = <BUS as sdmmc::bus::Bus>::Error> + SdSpiBus + 'static,
    DET: embedded_hal::digital::InputPin + 'static,
{
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), ManagerError> {
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), ManagerError> {
        Ok(())
    }
}

struct WrapperWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> WrapperWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }
}

impl<'a> core::fmt::Write for WrapperWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let rem = self.buf.len().saturating_sub(self.pos);
        let len = bytes.len().min(rem);
        self.buf[self.pos..self.pos + len].copy_from_slice(&bytes[..len]);
        self.pos += len;
        Ok(())
    }
}
