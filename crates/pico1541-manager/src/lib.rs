#![no_std]
#![allow(async_fn_in_trait)]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub mod driver;
pub mod tasks;

pub use driver::{StorageDriver, DisplayDriver, InputDriver, IndicatorDriver};

// Disk format code lives in pico1541-common; re-exported here for API compatibility.
pub use pico1541_common::{DiskImage, D64Image, G64Image, AnyDiskImage, Error, MemoryStorage};
pub use pico1541_common::gcr;
pub use pico1541_common::interface::{DriveEvent, ActiveTrack};

/// Inter-core communication channel for events from Emulator to Manager.
pub static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, DriveEvent, 64> = Channel::new();
