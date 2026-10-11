use crate::driver::blockdevsdcard::SdBlockIo;
use embassy_rp::gpio::Input;
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Blocking, Spi};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Delay;
use pico1541_common::DriveLocation;
use pico1541_manager::driver::file_storage::{FatFsFileWrapper, StorageWrapper};
use pico1541_manager::driver::{FileInfo, FileStorageDriver};
use pico1541_manager::formats::Error as FormatError;
use pico1541_manager::storage::RuntimeStorage;
use pico1541_manager::storage::{mangle_filename, DirectoryEntry};
use pico1541_manager::tasks::mount_manager::{match_c64_filename, ChildInfo, FileOpener};

pub type MySpi = Spi<'static, SPI1, Blocking>;

impl crate::driver::blockdevsdcard::SpiFreq for MySpi {
    fn set_freq(&mut self, freq: u32) {
        self.set_frequency(freq);
    }
}

pub type MyCs = Output<'static>;
pub type MyDelay = Delay;
pub type MyDet = Input<'static>;
pub type MyRawBlockIo = SdBlockIo<MySpi, MyCs, MyDelay>;
pub type MyFile = FatFsFileWrapper<MyRawBlockIo, MyDet>;
pub type MyStorage = RuntimeStorage<'static, MyFile, crate::littlefs_storage::LittleFsFileHandle>;

pub struct PlatformStorageManager {
    pub storage: StorageWrapper<MyRawBlockIo, MyDet>,
    pub location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
}

impl FileOpener for PlatformStorageManager {
    type FileStorage = MyStorage;

    async fn open_file(&self, requested_petscii: &[u8]) -> Result<Self::FileStorage, FormatError> {
        let (dir_path, use_internal) = match *self.location.lock().await {
            DriveLocation::VirtualDirectory { ref path, use_internal } => (path.clone(), use_internal),
            _ => return Err(FormatError::FileNotFound),
        };

        let mut found_filename = heapless::String::<64>::new();
        let mut found = false;

        let find_file = |info: FileInfo| {
            if !found {
                let mangled = mangle_filename(&info.name);
                if match_c64_filename(&mangled, requested_petscii) {
                    let name_len = info
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(info.name.len());
                    if let Ok(s) = core::str::from_utf8(&info.name[..name_len]) {
                        let _ = found_filename.push_str(s);
                        found = true;
                    }
                }
            }
        };

        if use_internal {
            let mut internal = crate::littlefs_storage::LittleFsStorage;
            let _ = internal
                .list_dir_range(dir_path.as_str(), 0, 1000, find_file)
                .await;
        } else {
            let mut sd = self.storage;
            let _ = sd
                .list_dir_range(dir_path.as_str(), 0, 1000, find_file)
                .await;
        }

        if !found {
            return Err(FormatError::FileNotFound);
        }

        let mut full_path = heapless::String::<128>::new();
        if dir_path == "/" || dir_path.is_empty() {
            let _ = full_path.push_str("/");
            let _ = full_path.push_str(found_filename.as_str());
        } else {
            let _ = full_path.push_str(&dir_path);
            let _ = full_path.push_str("/");
            let _ = full_path.push_str(found_filename.as_str());
        }

        if use_internal {
            let internal = crate::littlefs_storage::LittleFsStorage;
            let file_handle = internal.open_file_handle(full_path.as_str());
            Ok(RuntimeStorage::File2(file_handle))
        } else {
            let sd = self.storage;
            let file_handle = sd.open_file_handle(full_path.as_str());
            Ok(RuntimeStorage::File(file_handle))
        }
    }

    fn open_sd_file(&self, path: &str) -> Self::FileStorage {
        let sd = self.storage;
        RuntimeStorage::File(sd.open_file_handle(path))
    }

    fn open_internal_file(&self, path: &str) -> Self::FileStorage {
        let internal = crate::littlefs_storage::LittleFsStorage;
        RuntimeStorage::File2(internal.open_file_handle(path))
    }

    async fn list_dir_range(
        &self,
        path: &str,
        use_internal: bool,
        start_index: usize,
        count: usize,
        mut callback: impl FnMut(DirectoryEntry),
    ) -> Result<(), FormatError> {
        let handle_info = |info: FileInfo| {
            let name_len = info
                .name
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.name.len());
            let entry =
                DirectoryEntry::from_file_info(&info.name[..name_len], info.is_dir, info.size);
            callback(entry);
        };

        if use_internal {
            let mut internal = crate::littlefs_storage::LittleFsStorage;
            internal
                .list_dir_range(path, start_index, count, handle_info)
                .await
        } else {
            let mut storage = self.storage;
            storage
                .list_dir_range(path, start_index, count, handle_info)
                .await
        }
    }

    async fn find_child(
        &self,
        dir_path: &str,
        use_internal: bool,
        requested_petscii: &[u8],
    ) -> Result<Option<ChildInfo>, FormatError> {
        let mut found_child = None;

        let find_fn = |info: FileInfo| {
            if found_child.is_none() {
                let mangled = mangle_filename(&info.name);
                if match_c64_filename(&mangled, requested_petscii) {
                    let name_len = info
                        .name
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(info.name.len());
                    if let Ok(s) = core::str::from_utf8(&info.name[..name_len]) {
                        let mut name = heapless::String::<64>::new();
                        let _ = name.push_str(s);
                        found_child = Some(ChildInfo {
                            name,
                            is_dir: info.is_dir,
                        });
                    }
                }
            }
        };

        if use_internal {
            let mut internal = crate::littlefs_storage::LittleFsStorage;
            let _ = internal
                .list_dir_range(dir_path, 0, 1000, find_fn)
                .await;
        } else {
            let mut sd = self.storage;
            let _ = sd
                .list_dir_range(dir_path, 0, 1000, find_fn)
                .await;
        }

        Ok(found_child)
    }
}
