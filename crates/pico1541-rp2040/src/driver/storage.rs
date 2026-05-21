use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager, VolumeIdx, Mode, ShortFileName, Directory, LfnBuffer};
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

pub struct SdStorage<SPI, D, DET> 
where 
    SPI: embedded_hal::spi::SpiDevice,
    D: embedded_hal::delay::DelayNs,
    DET: embedded_hal::digital::InputPin
{
    pub volume_mgr: VolumeManager<SdCard<SPI, D>, DummyTimeSource>,
    pub det_pin: DET,
    pub volume_label: Option<heapless::String<11>>,
}

pub struct StorageWrapper<SPI, D, DET>(
    pub &'static embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, SdStorage<SPI, D, DET>>,
)
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static;

impl<SPI, D, DET> Clone for StorageWrapper<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<SPI, D, DET> Copy for StorageWrapper<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{}

pub struct SdFileHandle<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    storage: StorageWrapper<SPI, D, DET>,
    path: String<128>,
    offset: u64,
}

impl<SPI, D, DET> ErrorType for SdFileHandle<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    type Error = CommonError;
}

fn find_file_in_dir<SPI, D>(
    dir: &mut Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>,
    filename_str: &str,
) -> Result<ShortFileName, CommonError>
where
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
{
    let mut found_sfn = None;
    let mut lfn_store = [0u8; 256];
    let mut lfn_buffer = LfnBuffer::new(&mut lfn_store);

    let _ = dir.iterate_dir_lfn(&mut lfn_buffer, |entry, lfn| {
        if !entry.attributes.is_directory() {
            let mut sfn_buf = [0u8; 13];
            let mut writer = WrapperWriter::new(&mut sfn_buf);
            let _ = core::fmt::write(&mut writer, format_args!("{}", entry.name));
            let sfn_matches = if let Ok(sfn_str) = core::str::from_utf8(&sfn_buf) {
                sfn_str.trim_matches('\0').eq_ignore_ascii_case(filename_str)
            } else {
                false
            };

            let name_matches = sfn_matches || if let Some(lfn_str) = lfn {
                lfn_str.eq_ignore_ascii_case(filename_str)
            } else {
                false
            };

            if name_matches {
                found_sfn = Some(entry.name.clone());
            }
        }
    });

    found_sfn.ok_or(CommonError::Io)
}

impl<SPI, D, DET> Read for SdFileHandle<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut storage = self.storage.0.lock().await;
        if storage.det_pin.is_high().map_err(|_| CommonError::Io)? {
            return Err(CommonError::Io);
        }
        
        let (parent_path, filename_str) = if let Some(idx) = self.path.rfind('/') {
            (&self.path[..idx], &self.path[idx + 1..])
        } else {
            ("", self.path.as_str())
        };
        
        let offset = self.offset;
        let mut read_len = 0;
        
        with_dir_path_common(&mut *storage, parent_path, |dir| {
            let filename = find_file_in_dir(dir, filename_str)?;
            let file = dir.open_file_in_dir(filename, Mode::ReadOnly).map_err(|_| CommonError::Io)?;
            file.seek_from_start(offset as u32).map_err(|_| CommonError::Io)?;
            read_len = file.read(buf).map_err(|_| CommonError::Io)?;
            Ok(())
        })?;
        
        self.offset += read_len as u64;
        Ok(read_len)
    }
}

impl<SPI, D, DET> Seek for SdFileHandle<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
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
                
                let (parent_path, filename_str) = if let Some(idx) = self.path.rfind('/') {
                    (&self.path[..idx], &self.path[idx + 1..])
                } else {
                    ("", self.path.as_str())
                };
                
                let mut len = 0;
                with_dir_path_common(&mut *storage, parent_path, |dir| {
                    let filename = find_file_in_dir(dir, filename_str)?;
                    let file = dir.open_file_in_dir(filename, Mode::ReadOnly).map_err(|_| CommonError::Io)?;
                    len = file.length() as i64;
                    Ok(())
                })?;
                
                self.offset = (len + n) as u64;
            }
        }
        Ok(self.offset)
    }
}

impl<SPI, D, DET> FileStorageDriver for StorageWrapper<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    type File = SdFileHandle<SPI, D, DET>;

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
        
        if _path == "/" {
            storage.volume_label = None;
        }
        
        let mut found_volume_label = None;
        
        let res = with_dir_path(&mut *storage, _path, |dir| {
            let mut lfn_store = [0u8; 256];
            let mut lfn_buffer = LfnBuffer::new(&mut lfn_store);
            let _ = dir.iterate_dir_lfn(&mut lfn_buffer, |entry, lfn| {
                if entry.attributes.is_volume() {
                    let mut label_buf = [0u8; 12];
                    let mut writer = WrapperWriter::new(&mut label_buf);
                    let _ = core::fmt::write(&mut writer, format_args!("{}", entry.name));
                    if let Ok(label_str) = core::str::from_utf8(&label_buf) {
                        let trimmed = label_str.trim_matches('\0').trim();
                        let mut label = heapless::String::new();
                        let _ = label.push_str(trimmed);
                        found_volume_label = Some(label);
                    }
                    return;
                }

                let mut info = FileInfo {
                    name: [0; 64],
                    size: entry.size,
                    is_dir: entry.attributes.is_directory(),
                };
                let mut writer = WrapperWriter::new(&mut info.name);
                if let Some(lfn_str) = lfn {
                    let _ = core::fmt::write(&mut writer, format_args!("{}", lfn_str));
                } else {
                    let _ = core::fmt::write(&mut writer, format_args!("{}", entry.name));
                }

                let name_len = info.name.iter().position(|&c| c == 0).unwrap_or(info.name.len());
                if let Ok(name_str) = core::str::from_utf8(&info.name[..name_len]) {
                    if name_str.starts_with('.') {
                        return;
                    }
                }

                callback(info);
            });
            Ok(())
        });
        
        if res.is_ok() && found_volume_label.is_some() {
            storage.volume_label = found_volume_label;
        }
        res
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), ManagerError> {
        Ok(())
    }

    fn open_file_handle(&self, path: &str) -> Self::File {
        use core::str::FromStr;
        SdFileHandle {
            storage: *self,
            path: String::from_str(path).unwrap_or_default(),
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

impl<SPI, D, DET> StorageWrapper<SPI, D, DET>
where
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
}
impl<SPI, D, DET> SdStorage<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice,
    D: embedded_hal::delay::DelayNs,
    DET: embedded_hal::digital::InputPin
{
    pub fn new(spi_device: SPI, delayer: D, det_pin: DET) -> Self {
        let sdcard = SdCard::new(spi_device, delayer);
        let volume_mgr = VolumeManager::new(sdcard, DummyTimeSource);
        Self { volume_mgr, det_pin, volume_label: None }
    }
}

impl<SPI, D, DET> StorageDriver for SdStorage<SPI, D, DET>
where 
    SPI: embedded_hal::spi::SpiDevice,
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

fn with_dir_path<SPI, D, DET, R>(
    storage: &mut SdStorage<SPI, D, DET>,
    path: &str,
    f: impl FnOnce(&mut Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>) -> Result<R, ManagerError>
) -> Result<R, ManagerError>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    let volume = storage.volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| ManagerError::Io)?;
    let root = volume.open_root_dir().map_err(|_| ManagerError::Io)?;
    
    fn recurse<SPI, D, DET, R>(
        mut dir: Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>,
        mut segments: core::str::Split<'_, char>,
        f: impl FnOnce(&mut Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>) -> Result<R, ManagerError>
    ) -> Result<R, ManagerError>
    where
        SPI: embedded_hal::spi::SpiDevice + 'static,
        D: embedded_hal::delay::DelayNs + 'static
    {
        if let Some(segment) = segments.next() {
            if segment.is_empty() {
                recurse::<SPI, D, DET, R>(dir, segments, f)
            } else {
                let mut found_sfn = None;
                let mut lfn_store = [0u8; 256];
                let mut lfn_buffer = LfnBuffer::new(&mut lfn_store);
                
                let _ = dir.iterate_dir_lfn(&mut lfn_buffer, |entry, lfn| {
                    if entry.attributes.is_directory() {
                        let mut sfn_buf = [0u8; 13];
                        let mut writer = WrapperWriter::new(&mut sfn_buf);
                        let _ = core::fmt::write(&mut writer, format_args!("{}", entry.name));
                        let sfn_matches = if let Ok(sfn_str) = core::str::from_utf8(&sfn_buf) {
                            sfn_str.trim_matches('\0').eq_ignore_ascii_case(segment)
                        } else {
                            false
                        };

                        let name_matches = sfn_matches || if let Some(lfn_str) = lfn {
                            lfn_str.eq_ignore_ascii_case(segment)
                        } else {
                            false
                        };

                        if name_matches {
                            found_sfn = Some(entry.name.clone());
                        }
                    }
                });

                if let Some(sfn) = found_sfn {
                    let sub = dir.open_dir(sfn).map_err(|_| ManagerError::Io)?;
                    recurse::<SPI, D, DET, R>(sub, segments, f)
                } else {
                    Err(ManagerError::Io)
                }
            }
        } else {
            f(&mut dir)
        }
    }
    
    recurse::<SPI, D, DET, R>(root, path.split('/'), f)
}

fn with_dir_path_common<SPI, D, DET, R>(
    storage: &mut SdStorage<SPI, D, DET>,
    path: &str,
    f: impl FnOnce(&mut Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>) -> Result<R, CommonError>
) -> Result<R, CommonError>
where 
    SPI: embedded_hal::spi::SpiDevice + 'static,
    D: embedded_hal::delay::DelayNs + 'static,
    DET: embedded_hal::digital::InputPin + 'static
{
    let volume = storage.volume_mgr.open_volume(VolumeIdx(0)).map_err(|_| CommonError::Io)?;
    let root = volume.open_root_dir().map_err(|_| CommonError::Io)?;
    
    fn recurse<SPI, D, DET, R>(
        mut dir: Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>,
        mut segments: core::str::Split<'_, char>,
        f: impl FnOnce(&mut Directory<'_, SdCard<SPI, D>, DummyTimeSource, 4, 4, 1>) -> Result<R, CommonError>
    ) -> Result<R, CommonError>
    where
        SPI: embedded_hal::spi::SpiDevice + 'static,
        D: embedded_hal::delay::DelayNs + 'static
    {
        if let Some(segment) = segments.next() {
            if segment.is_empty() {
                recurse::<SPI, D, DET, R>(dir, segments, f)
            } else {
                let mut found_sfn = None;
                let mut lfn_store = [0u8; 256];
                let mut lfn_buffer = LfnBuffer::new(&mut lfn_store);
                
                let _ = dir.iterate_dir_lfn(&mut lfn_buffer, |entry, lfn| {
                    if entry.attributes.is_directory() {
                        let mut sfn_buf = [0u8; 13];
                        let mut writer = WrapperWriter::new(&mut sfn_buf);
                        let _ = core::fmt::write(&mut writer, format_args!("{}", entry.name));
                        let sfn_matches = if let Ok(sfn_str) = core::str::from_utf8(&sfn_buf) {
                            sfn_str.trim_matches('\0').eq_ignore_ascii_case(segment)
                        } else {
                            false
                        };

                        let name_matches = sfn_matches || if let Some(lfn_str) = lfn {
                            lfn_str.eq_ignore_ascii_case(segment)
                        } else {
                            false
                        };

                        if name_matches {
                            found_sfn = Some(entry.name.clone());
                        }
                    }
                });

                if let Some(sfn) = found_sfn {
                    let sub = dir.open_dir(sfn).map_err(|_| CommonError::Io)?;
                    recurse::<SPI, D, DET, R>(sub, segments, f)
                } else {
                    Err(CommonError::Io)
                }
            }
        } else {
            f(&mut dir)
        }
    }
    
    recurse::<SPI, D, DET, R>(root, path.split('/'), f)
}
