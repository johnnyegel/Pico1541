#![no_std]

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

// Disk format code lives in pico1541-common; re-exported here for API compatibility.
pub use pico1541_common::{DiskImage, D64Image, G64Image, AnyDiskImage, Error, MemoryStorage};
pub use pico1541_common::gcr;
pub use pico1541_common::interface::{DriveEvent, ActiveTrack};

/// Driver abstraction for the manager.
pub trait StorageDriver {
    async fn read_block(&mut self, track: u8, sector: u8, buf: &mut [u8]) -> Result<(), Error>;
    async fn write_block(&mut self, track: u8, sector: u8, buf: &[u8]) -> Result<(), Error>;
}

pub trait DisplayDriver {
    async fn clear(&mut self);
    async fn print(&mut self, line: u8, text: &str);
}

pub trait InputDriver {
    async fn wait_for_button(&mut self) -> u8;
}

/// The Manager state and logic.
pub struct Manager<S: StorageDriver, D: DisplayDriver, I: InputDriver> {
    storage: S,
    display: D,
    input: I,
}

impl<S: StorageDriver, D: DisplayDriver, I: InputDriver> Manager<S, D, I> {
    pub fn new(storage: S, display: D, input: I) -> Self {
        Self { storage, display, input }
    }

    pub async fn run(&mut self) -> ! {
        self.display.clear().await;
        self.display.print(0, "Pico1541 Ready").await;

        loop {
            // Main manager loop
            let _btn = self.input.wait_for_button().await;
            self.display.print(1, "Button Pressed").await;
        }
    }
}

/// Inter-core communication channel for events from Emulator to Manager.
pub static EVENT_CHANNEL: Channel<CriticalSectionRawMutex, DriveEvent, 64> = Channel::new();
