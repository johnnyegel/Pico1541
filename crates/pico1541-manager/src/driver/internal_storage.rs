use crate::driver::{FileStorageDriver, FileInfo};
use pico1541_common::formats::Error;
use pico1541_common::storage::{MemoryStorage, Error as CommonError};
use embedded_io_async::{Read, Seek, SeekFrom, ErrorType};

pub struct InternalFile {
    pub name: &'static str,
    pub data: &'static [u8],
}

pub static INTERNAL_FILES: &[InternalFile] = &[
    InternalFile {
        name: "fb.prg",
        data: include_bytes!("../../../../internal-fs/fb.prg"),
    },
    InternalFile {
        name: "fb20fl.prg",
        data: include_bytes!("../../../../internal-fs/fb20fl.prg"),
    },
];

#[derive(Clone, Copy)]
pub struct InternalStorage;

impl FileStorageDriver for InternalStorage {
    async fn list_dir(&mut self, _path: &str, mut callback: impl FnMut(FileInfo)) -> Result<(), Error> {
        for file in INTERNAL_FILES {
            let mut info = FileInfo {
                name: [0; 13],
                size: file.data.len() as u32,
                is_dir: false,
            };
            let len = file.name.len().min(12);
            info.name[..len].copy_from_slice(&file.name.as_bytes()[..len]);
            callback(info);
        }
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), Error> {
        Ok(())
    }
}

pub struct InternalFileHandle {
    storage: MemoryStorage<'static>,
}

impl ErrorType for InternalFileHandle {
    type Error = CommonError;
}

impl Read for InternalFileHandle {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.storage.read(buf).await.map_err(|_| CommonError::Io)
    }
}

impl Seek for InternalFileHandle {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        self.storage.seek(pos).await.map_err(|_| CommonError::Io)
    }
}

impl InternalStorage {
    pub fn open_file_handle(&self, name: &str) -> Option<InternalFileHandle> {
        for file in INTERNAL_FILES {
            if file.name == name {
                return Some(InternalFileHandle {
                    storage: MemoryStorage::new(file.data),
                });
            }
        }
        None
    }
}
