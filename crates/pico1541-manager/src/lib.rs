#![no_std]

// Disk format code lives in pico1541-common; re-exported here for API compatibility.
pub use pico1541_common::{DiskImage, D64Image, G64Image, AnyDiskImage, Error, MemoryStorage};
pub use pico1541_common::gcr;
