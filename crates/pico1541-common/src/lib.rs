#![no_std]

pub mod gcr;
pub mod formats;
pub mod storage;
pub mod disk_info;

pub use formats::{DiskImage, D64Image, G64Image, AnyDiskImage, Error};
pub use storage::MemoryStorage;
