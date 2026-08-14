#![cfg_attr(not(test), no_std)]

pub mod context;
pub mod disk_info;
pub mod iec_com;
pub mod interface;
pub mod snooper;
pub mod track_buffer;
pub mod magnetic_surface;
pub mod util;

pub use iec_com::{IecComHostBus, IecCommander, IecError, IecTransaction};
pub use interface::{DriveEvent, HostInterface};

pub const SYSTEM_CLOCK_FREQ: u32 = 250_000_000;
