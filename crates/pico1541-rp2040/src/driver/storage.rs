use pico1541_manager::driver::{StorageDriver, FileStorageDriver, FileInfo};
use pico1541_common::formats::Error as ManagerError;
use pico1541_common::storage::Error as CommonError;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use hadris_fat::r#async::fs::FatFs;
use hadris_fat::r#async::dir::DirectoryEntry;
use hadris_fat::DirEntryAttrFlags;
use hadris_io::SeekFrom;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::spi::SpiBus;

use crate::driver::sdcard::{SdCard, SdSpi};

pub struct SdBlockIo<SPI, CS> {
    card_ptr: *mut SdCard<SPI, CS>,
    offset: u64,
    cache: [u8; 512],
    cached_sector: Option<u32>,
}

unsafe impl<SPI, CS> Send for SdBlockIo<SPI, CS> {}
unsafe impl<SPI, CS> Sync for SdBlockIo<SPI, CS> {}

impl<SPI, CS> SdBlockIo<SPI, CS> {
    pub fn new(card_ptr: *mut SdCard<SPI, CS>) -> Self {
        Self {
            card_ptr,
            offset: 0,
            cache: [0u8; 512],
            cached_sector: None,
        }
    }
}

impl<SPI, CS> hadris_io::r#async::Read for SdBlockIo<SPI, CS>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, hadris_io::Error> {
        let mut total_read = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_read < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_read).min(bytes_avail);
            
            if self.cached_sector != Some(sector) {
                let mut sector_data = [0u8; 512];
                card.read_block(sector, &mut sector_data).await.map_err(|e| {
                    defmt::error!("SdBlockIo: read_block failed: {:?}", e);
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

impl<SPI, CS> hadris_io::r#async::Seek for SdBlockIo<SPI, CS>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
{
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, hadris_io::Error> {
        let card = unsafe { &mut *self.card_ptr };
        let new_offset = match pos {
            SeekFrom::Start(n) => n,
            SeekFrom::Current(n) => (self.offset as i64 + n) as u64,
            SeekFrom::End(n) => {
                let num_blocks = card.num_blocks() as u64;
                let total_bytes = num_blocks * 512;
                (total_bytes as i64 + n) as u64
            }
        };
        self.offset = new_offset;
        Ok(new_offset)
    }
}

impl<SPI, CS> hadris_io::r#async::Write for SdBlockIo<SPI, CS>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, hadris_io::Error> {
        let mut total_written = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_written < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_written).min(bytes_avail);

            if self.cached_sector != Some(sector) {
                let mut sector_data = [0u8; 512];
                if card.read_block(sector, &mut sector_data).await.is_ok() {
                    self.cache = sector_data;
                } else {
                    self.cache = [0u8; 512];
                }
                self.cached_sector = Some(sector);
            }

            self.cache[offset_in_sector..offset_in_sector + to_copy]
                .copy_from_slice(&buf[total_written..total_written + to_copy]);
            
            card.write_block(sector, &self.cache).await.map_err(|e| {
                defmt::error!("SdBlockIo: write_block failed: {:?}", e);
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

pub struct SdStorage<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    pub card: SdCard<SPI, CS>,
    pub card_init: bool,
    pub fat_fs: Option<FatFs<SdBlockIo<SPI, CS>>>,
    pub det_pin: DET,
    pub volume_label: Option<heapless::String<11>>,
}

impl<SPI, CS, DET> SdStorage<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    pub fn new(spi: SPI, cs: CS, det_pin: DET) -> Self {
        Self {
            card: SdCard::new(spi, cs),
            card_init: false,
            fat_fs: None,
            det_pin,
            volume_label: None,
        }
    }

    pub async fn ensure_mounted(&mut self) -> Result<&FatFs<SdBlockIo<SPI, CS>>, ManagerError> {
        if self.det_pin.is_high().map_err(|_| ManagerError::Io)? {
            self.fat_fs = None;
            self.card_init = false;
            return Err(ManagerError::Io);
        }

        if self.fat_fs.is_none() || !self.card_init {
            self.fat_fs = None;
            self.card_init = false;
            
            defmt::info!("SD: initiating card...");
            if let Err(e) = self.card.init().await {
                defmt::error!("SD: init failed: {:?}", e);
                return Err(ManagerError::Io);
            }
            self.card_init = true;
            defmt::info!("SD: card initialized successfully.");

            let block_io = SdBlockIo::new(&mut self.card as *mut SdCard<SPI, CS>);
            defmt::info!("SD: opening FatFs...");
            let fs = match FatFs::open(block_io).await {
                Ok(f) => f,
                Err(e) => {
                    defmt::error!("SD: FatFs::open failed: {:?}", defmt::Debug2Format(&e));
                    self.card_init = false;
                    return Err(ManagerError::Io);
                }
            };
            self.fat_fs = Some(fs);
            defmt::info!("SD: mounted successfully!");
        }

        Ok(self.fat_fs.as_ref().unwrap())
    }
}

pub struct StorageWrapper<SPI, CS, DET>(
    pub &'static Mutex<CriticalSectionRawMutex, SdStorage<SPI, CS, DET>>,
)
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static;

impl<SPI, CS, DET> Clone for StorageWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<SPI, CS, DET> Copy for StorageWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{}

pub struct HadrisFileWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    storage: StorageWrapper<SPI, CS, DET>,
    path: heapless::String<128>,
    offset: u64,
}

impl<SPI, CS, DET> embedded_io_async::ErrorType for HadrisFileWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    type Error = CommonError;
}

impl<SPI, CS, DET> embedded_io_async::Read for HadrisFileWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
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

impl<SPI, CS, DET> embedded_io_async::Seek for HadrisFileWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
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

impl<SPI, CS, DET> FileStorageDriver for StorageWrapper<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
{
    type File = HadrisFileWrapper<SPI, CS, DET>;

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

impl<SPI, CS, DET> StorageDriver for SdStorage<SPI, CS, DET>
where
    SPI: SpiBus<u8> + SdSpi + 'static,
    CS: OutputPin + 'static,
    DET: InputPin + 'static,
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
