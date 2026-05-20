#![cfg_attr(not(test), no_std)]
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
    LongPress(Button),
}

/// Channel for keyboard events (logical button events).
pub static KEYBOARD_CHANNEL: Channel<CriticalSectionRawMutex, ButtonEvent, 16> = Channel::new();
