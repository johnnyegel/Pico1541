#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

pub mod driver;
pub mod tasks;

pub use driver::{DisplayDriver, IndicatorDriver, InputDriver, RgbLedDriver, StatusLedMode, StorageDriver};

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
    HeadStepped(u8, u8, u8),
    CloseChannel(u8),
    ImageMounted,
    ImageUnmounted,
    MotorState(bool),
}

pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, InternalCommand, 128> = Channel::new();

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

pub static KEYBOARD_CHANNEL: Channel<CriticalSectionRawMutex, ButtonEvent, 16> = Channel::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum UiAction {
    ScrollUp,
    ScrollDown,
    Select,
    Back,
    SkipUp,
    SkipDown,
    Home,
    Monitor,
    ContextAction,
}

pub static ACTION_CHANNEL: Channel<CriticalSectionRawMutex, UiAction, 16> = Channel::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsCommand {
    SaveDriveId(u8),
    SaveBootRom(heapless::String<32>),
    DeleteRom(heapless::String<32>),
    InstallRomStart { filename: heapless::String<32>, size: u32, offset: u32 },
    InstallRomChunk(heapless::Vec<u8, 256>),
    InstallRomFinish,
    LoadRomToMemory(heapless::String<32>),
}
pub static SETTINGS_CHANNEL: Channel<CriticalSectionRawMutex, SettingsCommand, 2> = Channel::new();
pub static SETTINGS_SAVE_RESULT: portable_atomic::AtomicU8 = portable_atomic::AtomicU8::new(0);
pub static SETTINGS_SAVE_REQUEST: portable_atomic::AtomicU8 = portable_atomic::AtomicU8::new(0);


#[cfg(test)]
mod test_imports {
}

