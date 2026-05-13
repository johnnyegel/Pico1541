#![no_std]
#![allow(async_fn_in_trait)]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub mod driver;
pub mod tasks;

pub use driver::{ActivityColor, DisplayDriver, IndicatorDriver, InputDriver, RgbLedDriver, StorageDriver};

// Disk format code lives in pico1541-common; re-exported here for API compatibility.
pub use pico1541_common::gcr;
pub use pico1541_common::interface::{ActiveTrack, DriveEvent};
pub use pico1541_common::{AnyDiskImage, D64Image, DiskImage, Error, G64Image, MemoryStorage};

/// Inter-core communication channel for events from Emulator to Manager.
pub static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, DriveEvent, 64> = Channel::new();

/// Channel for high-level events (LEDs, Motor, Decoded Commands) handled by the Manager.
pub static ACTIVITY_CHANNEL: Channel<CriticalSectionRawMutex, DriveEvent, 16> = Channel::new();

#[derive(Clone, Copy, Debug)]
pub enum InternalCommand {
    LoadFile([u8; 16]),
    HeadStepped(u8),
}

pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, InternalCommand, 8> = Channel::new();

/// Channel for keyboard events (raw bitmask of pressed buttons).
pub static KEYBOARD_CHANNEL: Channel<CriticalSectionRawMutex, u8, 8> = Channel::new();
