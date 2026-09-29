#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;

pub mod driver;
pub mod formats;
pub mod gcr;
pub mod storage;
pub mod tasks;

pub use driver::{
    DisplayDriver, IndicatorDriver, InputDriver, RgbLedDriver, StatusLedMode, StorageDriver,
};

pub use formats::{AnyDiskImage, D64Image, DiskImage, Error, G64Image, NibImage};
pub use storage::{
    MemoryStorage, PagedDirectorySource, PagedDirectorySourceTrait, VectorDirectorySource,
};

use embedded_io_async::{Read, Seek};
use storage::DirectorySource;

/// Maps a mounted disk image to the corresponding status LED mode.
pub fn get_status_mode<S: Read + Seek, D: DirectorySource>(
    disk: Option<&AnyDiskImage<S, D>>,
) -> StatusLedMode {
    match disk {
        None => StatusLedMode::Unmounted,
        Some(AnyDiskImage::VirtualDisk(_)) => StatusLedMode::VirtualDisk,
        Some(AnyDiskImage::D64(_)) => StatusLedMode::NonAccurateDisk,
        Some(AnyDiskImage::G64(_)) | Some(AnyDiskImage::Nib(_)) => StatusLedMode::AccurateDisk,
    }
}

#[derive(Clone, Debug)]
pub enum MountPayload {
    VirtualDirectory(heapless::String<128>, bool),
    D64(heapless::String<128>, bool),
    G64(heapless::String<128>, bool),
    Nib(heapless::String<128>, bool),
    /// Single non-image file (PRG, SEQ, …) served via the track-1 treadmill.
    Prg(heapless::String<128>, bool),
}

impl MountPayload {
    pub fn path(&self) -> &str {
        match self {
            Self::VirtualDirectory(p, _)
            | Self::D64(p, _)
            | Self::G64(p, _)
            | Self::Nib(p, _)
            | Self::Prg(p, _) => p.as_str(),
        }
    }

    pub fn use_internal(&self) -> bool {
        match self {
            Self::VirtualDirectory(_, i)
            | Self::D64(_, i)
            | Self::G64(_, i)
            | Self::Nib(_, i)
            | Self::Prg(_, i) => *i,
        }
    }

    pub fn status_led_mode(&self) -> StatusLedMode {
        match self {
            Self::VirtualDirectory(..) | Self::Prg(..) => StatusLedMode::VirtualDisk,
            Self::D64(..) => StatusLedMode::NonAccurateDisk,
            Self::G64(..) | Self::Nib(..) => StatusLedMode::AccurateDisk,
        }
    }

    pub fn into_parts(self) -> (heapless::String<128>, bool, StatusLedMode) {
        let mode = self.status_led_mode();
        match self {
            Self::VirtualDirectory(p, i)
            | Self::D64(p, i)
            | Self::G64(p, i)
            | Self::Nib(p, i)
            | Self::Prg(p, i) => (p, i, mode),
        }
    }
}


#[derive(Clone, Debug)]
pub enum InternalCommand {
    MotorState(bool),
    Mount(MountPayload),
    Unmount,
    Cd(heapless::String<64>),
    SelectVirtualFile(usize),
}

pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, InternalCommand, 128> = Channel::new();
pub static LOCATION_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

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
    FastScrollUp,
    FastScrollDown,
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
#[allow(clippy::large_enum_variant)]
pub enum SettingsCommand {
    SaveDriveId(u8),
    SaveBootRom(heapless::String<32>),
    DeleteRom(heapless::String<32>),
    InstallRomStart {
        filename: heapless::String<32>,
        size: u32,
        offset: u32,
    },
    InstallRomChunk(heapless::Vec<u8, 256>),
    InstallRomFinish,
    LoadRomToMemory(heapless::String<32>),
}
pub static SETTINGS_CHANNEL: Channel<CriticalSectionRawMutex, SettingsCommand, 2> = Channel::new();

#[cfg(test)]
mod test_imports {}
