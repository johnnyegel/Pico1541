#![no_std]

pub mod gcr;
pub mod formats;
pub mod storage;

pub use formats::{DiskImage, D64Image, G64Image, Error};
pub use storage::MemoryStorage;
