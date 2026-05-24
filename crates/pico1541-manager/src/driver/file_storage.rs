use crate::driver::{FileStorageDriver, FileInfo, StorageDriver};
use pico1541_common::formats::Error as ManagerError;
use pico1541_common::storage::Error as CommonError;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use hadris_fat::r#async::fs::FatFs;
use hadris_fat::r#async::dir::DirectoryEntry;
use hadris_fat::DirEntryAttrFlags;
use hadris_io::SeekFrom;
use hadris_io::r#async::Seek;
use embedded_hal::digital::InputPin;

use hadris_part::r#async::scheme_io::DiskPartitionSchemeReadExt;
use hadris_part::scheme::DiskPartitionScheme;

fn normalize_path(path: &str) -> &str {
    let mut p = path;
    if p.starts_with('/') {
        p = &p[1..];
    }
    if p.ends_with('/') && !p.is_empty() {
        p = &p[..p.len() - 1];
    }
    p
}

pub struct PartitionedBlockDevice<B> {
    inner: B,
    partition_start_bytes: u64,
}

fn format_fat_name(name: &str) -> heapless::String<64> {
    let mut clean_name = heapless::String::<64>::new();
    if let Some((base, ext)) = name.split_once('.') {
        let base_trim = base.trim_end();
        let ext_trim = ext.trim_end();
        
        for c in base_trim.chars() {
            if clean_name.push(c).is_err() { break; }
        }
        
        if !ext_trim.is_empty() {
            if clean_name.push('.').is_ok() {
                for c in ext_trim.chars() {
                    if clean_name.push(c).is_err() { break; }
                }
            }
        }
    } else {
        for c in name.trim_end().chars() {
            if clean_name.push(c).is_err() { break; }
        }
    }
    clean_name
}

impl<B> PartitionedBlockDevice<B> {
    pub fn new(inner: B, partition_start_sectors: u64) -> Self {
        Self {
            inner,
            partition_start_bytes: partition_start_sectors * 512,
        }
    }
}

impl<B: hadris_io::r#async::Read> hadris_io::r#async::Read for PartitionedBlockDevice<B> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, hadris_io::Error> {
        self.inner.read(buf).await
    }
}

impl<B: hadris_io::r#async::Write> hadris_io::r#async::Write for PartitionedBlockDevice<B> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, hadris_io::Error> {
        self.inner.write(buf).await
    }
    async fn flush(&mut self) -> Result<(), hadris_io::Error> {
        self.inner.flush().await
    }
}

impl<B: hadris_io::r#async::Seek> hadris_io::r#async::Seek for PartitionedBlockDevice<B> {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, hadris_io::Error> {
        let physical_pos = match pos {
            SeekFrom::Start(n) => SeekFrom::Start(n + self.partition_start_bytes),
            SeekFrom::Current(n) => SeekFrom::Current(n),
            SeekFrom::End(n) => SeekFrom::End(n),
        };
        let res = self.inner.seek(physical_pos).await?;
        if res < self.partition_start_bytes {
            return Err(hadris_io::Error::new_static(
                hadris_io::ErrorKind::Other,
                "Seek out of partition bounds",
            ));
        }
        Ok(res - self.partition_start_bytes)
    }
}

pub struct FatStorage<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    pub block_dev: B,
    pub card_init: bool,
    pub fat_fs: Option<FatFs<PartitionedBlockDevice<B>>>,
    pub det_pin: Det,
    pub volume_label: Option<heapless::String<11>>,
}

impl<B, Det> FatStorage<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    pub fn new(block_dev: B, det_pin: Det) -> Self {
        Self {
            block_dev,
            card_init: false,
            fat_fs: None,
            det_pin,
            volume_label: None,
        }
    }

    pub async fn ensure_mounted(&mut self) -> Result<&FatFs<PartitionedBlockDevice<B>>, ManagerError> {
        if self.det_pin.is_high().map_err(|_| ManagerError::Io)? {
            self.fat_fs = None;
            self.card_init = false;
            return Err(ManagerError::Io);
        }

        if self.fat_fs.is_none() || !self.card_init {
            self.fat_fs = None;
            self.card_init = false;

            #[cfg(feature = "defmt")]
            defmt::info!("SD: checking partition table via hadris-part...");

            // Reset raw seek offset before reading partitions
            let mut raw_dev = self.block_dev.clone();
            let _ = raw_dev.seek(SeekFrom::Start(0)).await.map_err(|_| ManagerError::Io)?;

            // Use hadris-part to detect and parse the partition scheme
            let partition_start = if let Ok(scheme) = DiskPartitionScheme::read_from(&mut raw_dev, 512).await {
                if let Some(part) = scheme.partitions().first() {
                    #[cfg(feature = "defmt")]
                    defmt::info!("SD: Partition table detected. Partition 1 LBA start: {}", part.start_lba);
                    part.start_lba
                } else {
                    #[cfg(feature = "defmt")]
                    defmt::info!("SD: No partitions found in partition table. Defaulting to sector 0.");
                    0
                }
            } else {
                #[cfg(feature = "defmt")]
                defmt::info!("SD: No valid partition table detected. Accessing sector 0 directly.");
                0
            };

            let mut partitioned_dev = PartitionedBlockDevice::new(self.block_dev.clone(), partition_start);
            // Seek to 0 on the partitioned device
            let _ = partitioned_dev.seek(SeekFrom::Start(0)).await.map_err(|_| ManagerError::Io)?;

            #[cfg(feature = "defmt")]
            defmt::info!("SD: opening FatFs...");

            let fs = match FatFs::open(partitioned_dev).await {
                Ok(f) => f,
                Err(e) => {
                    #[cfg(feature = "defmt")]
                    defmt::error!("SD: FatFs::open failed: {:?}", defmt::Debug2Format(&e));
                    self.card_init = false;
                    return Err(ManagerError::Io);
                }
            };
            self.card_init = true;
            self.fat_fs = Some(fs);

            #[cfg(feature = "defmt")]
            defmt::info!("SD: mounted successfully!");
        }

        Ok(self.fat_fs.as_ref().unwrap())
    }
}

pub struct StorageWrapper<B, Det>(
    pub &'static Mutex<CriticalSectionRawMutex, FatStorage<B, Det>>,
)
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static;

impl<B, Det> Clone for StorageWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<B, Det> Copy for StorageWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{}

pub struct HadrisFileWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    storage: StorageWrapper<B, Det>,
    path: heapless::String<128>,
    offset: u64,
}

impl<B, Det> embedded_io_async::ErrorType for HadrisFileWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    type Error = CommonError;
}

impl<B, Det> embedded_io_async::Read for HadrisFileWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut storage = self.storage.0.lock().await;
        let fs = storage.ensure_mounted().await.map_err(|_| CommonError::Io)?;
        
        let mut file_reader = fs.open_file_path(normalize_path(self.path.as_str())).await.map_err(|_| CommonError::Io)?;
        
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

impl<B, Det> embedded_io_async::Seek for HadrisFileWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    async fn seek(&mut self, pos: embedded_io_async::SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            embedded_io_async::SeekFrom::Start(n) => self.offset = n,
            embedded_io_async::SeekFrom::Current(n) => self.offset = (self.offset as i64 + n) as u64,
            embedded_io_async::SeekFrom::End(n) => {
                let mut storage = self.storage.0.lock().await;
                let fs = storage.ensure_mounted().await.map_err(|_| CommonError::Io)?;
                let file_reader = fs.open_file_path(normalize_path(self.path.as_str())).await.map_err(|_| CommonError::Io)?;
                let len = file_reader.size() as i64;
                self.offset = (len + n) as u64;
            }
        }
        Ok(self.offset)
    }
}

impl<B, Det> FileStorageDriver for StorageWrapper<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    type File = HadrisFileWrapper<B, Det>;

    async fn is_available(&self) -> bool {
        if let Ok(storage) = self.0.try_lock() {
            storage.fat_fs.is_some()
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
            #[cfg(feature = "defmt")]
            defmt::info!("SD: list_dir path: '{}'", path);

            let normalized = normalize_path(path);
            let dir = if normalized.is_empty() {
                fs.root_dir()
            } else {
                match fs.open_dir_path(normalized).await {
                    Ok(d) => d,
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        defmt::error!("SD: list_dir open_dir_path('{}') failed: {:?}", normalized, defmt::Debug2Format(&e));
                        return Err(ManagerError::Io);
                    }
                }
            };
            let mut iter = dir.entries();
            
            while let Some(entry_res) = iter.next_entry().await {
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        defmt::error!("SD: list_dir next_entry failed: {:?}", defmt::Debug2Format(&e));
                        return Err(ManagerError::Io);
                    }
                };
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
                let clean_name = format_fat_name(name);
                let mut writer = WrapperWriter::new(&mut info.name);
                let _ = core::fmt::write(&mut writer, format_args!("{}", clean_name.as_str()));
                callback(info);
            }
        }
        
        if temp_volume_label.is_some() {
            storage.volume_label = temp_volume_label;
        }
        
        Ok(())
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
            #[cfg(feature = "defmt")]
            defmt::info!("SD: list_dir_range path: '{}'", path);

            let normalized = normalize_path(path);
            let dir = if normalized.is_empty() {
                fs.root_dir()
            } else {
                match fs.open_dir_path(normalized).await {
                    Ok(d) => d,
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        defmt::error!("SD: list_dir_range open_dir_path('{}') failed: {:?}", normalized, defmt::Debug2Format(&e));
                        return Err(ManagerError::Io);
                    }
                }
            };
            let mut iter = dir.entries();
            
            while let Some(entry_res) = iter.next_entry().await {
                if current_idx >= start_index + count {
                    break;
                }

                let entry = match entry_res {
                    Ok(e) => e,
                    Err(e) => {
                        #[cfg(feature = "defmt")]
                        defmt::error!("SD: list_dir_range next_entry failed: {:?}", defmt::Debug2Format(&e));
                        return Err(ManagerError::Io);
                    }
                };
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
                    let clean_name = format_fat_name(name);
                    let mut writer = WrapperWriter::new(&mut info.name);
                    let _ = core::fmt::write(&mut writer, format_args!("{}", clean_name.as_str()));
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

impl<B, Det> StorageDriver for FatStorage<B, Det>
where
    B: hadris_io::r#async::Read + hadris_io::r#async::Write + hadris_io::r#async::Seek + Send + Sync + Clone + 'static,
    Det: InputPin + 'static,
{
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), ManagerError> {
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), ManagerError> {
        Ok(())
    }
}
