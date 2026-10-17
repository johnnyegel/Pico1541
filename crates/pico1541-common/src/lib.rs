#![cfg_attr(not(test), no_std)]

pub mod context;
pub mod disk_info;
pub mod iec_com;
pub mod interface;
pub mod location;
pub mod magnetic_surface;
pub mod snooper;
pub mod sound;
pub mod track_buffer;
pub mod util;

pub use iec_com::{IecComHostBus, IecCommander, IecError, IecTransaction};
pub use interface::{DriveEvent, HostInterface};
pub use location::DriveLocation;
pub use track_buffer::{
    slot_to_track_sector, track_sector_to_slot, Lfsr16, WeakBitRange, MAX_WEAK_RANGES,
};

pub static HALT_EMULATOR: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);
pub static VIRTUAL_FS_ACTIVE: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(true);
