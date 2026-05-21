#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub mod driver;
pub mod tasks;

pub use driver::{ActivityColor, DisplayDriver, IndicatorDriver, InputDriver, RgbLedDriver, StatusLedMode, StorageDriver};

// Disk format code lives in pico1541-common; re-exported here for API compatibility.
pub use pico1541_common::gcr;
pub use pico1541_common::interface::{ActiveTrack, DriveEvent};
pub use pico1541_common::{AnyDiskImage, D64Image, DiskImage, Error, G64Image, MemoryStorage, VectorDirectorySource, PagedDirectorySource, PagedDirectorySourceTrait};


use embedded_io_async::{Read, Seek};
use pico1541_common::storage::DirectorySource;

/// Maps a mounted disk image to the corresponding status LED mode.
pub fn get_status_mode<S: Read + Seek, D: DirectorySource>(
    disk: Option<&AnyDiskImage<S, D>>,
) -> StatusLedMode {
    match disk {
        None => StatusLedMode::Unmounted,
        Some(AnyDiskImage::VirtualDisk(_)) => StatusLedMode::VirtualDisk,
        Some(AnyDiskImage::D64(_)) => StatusLedMode::NonAccurateDisk,
        Some(AnyDiskImage::G64(_)) => StatusLedMode::AccurateDisk,
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Button {
    Up,
    Down,
    Select,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ButtonEvent {
    Pressed(Button),
    Released(Button),
    Hold(Button),
}

/// Channel for keyboard events (logical button events).
pub static KEYBOARD_CHANNEL: Channel<CriticalSectionRawMutex, ButtonEvent, 16> = Channel::new();
