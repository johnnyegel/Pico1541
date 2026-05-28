use pico1541_manager::driver::{FileStorageDriver, FileInfo};
use pico1541_common::formats::Error;
use pico1541_common::storage::Error as CommonError;
use embedded_io_async::{Read, Seek, SeekFrom, ErrorType};
use littlefs_rust::{Filesystem, OpenFlags};
use crate::flash_storage::ReadOnlyFlash;
use heapless::String;

#[derive(Clone, Copy)]
pub struct LittleFsStorage;

impl FileStorageDriver for LittleFsStorage {
    type File = LittleFsFileHandle;

    async fn is_available(&self) -> bool {
        true
    }

    async fn list_dir(&mut self, path: &str, mut callback: impl FnMut(FileInfo)) -> Result<(), Error> {
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        
        let flash = ReadOnlyFlash::new();
        if let Ok(fs) = Filesystem::mount(flash, config) {
            if let Ok(dir) = fs.read_dir(path) {
                for entry in dir {
                    if let Ok(entry) = entry {
                        let name_str = entry.name.as_str();
                        if name_str.starts_with('.') {
                            continue;
                        }
                        let mut info = FileInfo {
                            name: [0; 64],
                            size: entry.size,
                            is_dir: entry.file_type == littlefs_rust::FileType::Dir,
                        };
                        let bytes = name_str.as_bytes();
                        let len = bytes.len().min(63);
                        info.name[..len].copy_from_slice(&bytes[..len]);
                        callback(info);
                    }
                }
            }
            fs.unmount().unwrap();
        }
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), Error> {
        Ok(())
    }

    fn open_file_handle(&self, path: &str) -> Self::File {
        LittleFsFileHandle {
            path: core::str::FromStr::from_str(path).unwrap(),
            pos: 0,
        }
    }

    async fn free_space(&self) -> Option<u32> {
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        
        let flash = ReadOnlyFlash::new();
        if let Ok(fs) = Filesystem::mount(flash, config) {
            let allocated = fs.fs_size().unwrap_or(128);
            fs.unmount().unwrap();
            let free = 128u32.saturating_sub(allocated);
            Some(free * 4096)
        } else {
            None
        }
    }
}

pub struct LittleFsFileHandle {
    path: String<128>,
    pos: u32,
}

impl ErrorType for LittleFsFileHandle {
    type Error = CommonError;
}

impl Read for LittleFsFileHandle {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        
        let flash = ReadOnlyFlash::new();
        if let Ok(fs) = Filesystem::mount(flash, config) {
            if let Ok(file) = fs.open(self.path.as_str(), OpenFlags::READ) {
                if file.seek(littlefs_rust::SeekFrom::Start(self.pos)).is_ok() {
                    if let Ok(n) = file.read(buf) {
                        self.pos += n as u32;
                        return Ok(n as usize);
                    }
                }
            }
            fs.unmount().unwrap();
        }
        Err(CommonError::Io)
    }
}

impl Seek for LittleFsFileHandle {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        // Just calculate new pos based on SeekFrom
        let new_pos = match pos {
            SeekFrom::Start(p) => p as u32,
            SeekFrom::Current(p) => (self.pos as i64 + p) as u32,
            SeekFrom::End(p) => {
                // To seek from end, we must read the size first
                let mut config = littlefs_rust::Config::new(4096, 128);
                config.read_size = 256;
                config.prog_size = 256;
                config.cache_size = 256;
                config.lookahead_size = 16;
                let flash = ReadOnlyFlash::new();
                let mut size = 0;
                if let Ok(fs) = Filesystem::mount(flash, config) {
                    if let Ok(file) = fs.open(self.path.as_str(), OpenFlags::READ) {
                        size = file.size();
                    }
                }
                (size as i64 + p) as u32
            }
        };
        self.pos = new_pos;
        Ok(self.pos as u64)
    }
}
