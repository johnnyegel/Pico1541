pub mod d64;
pub mod g64;
pub mod virtual_disk;

pub use d64::D64Image;
pub use g64::G64Image;
pub use virtual_disk::VirtualDiskImage;

use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Io,
    InvalidFormat,
    Corrupted,
    InvalidTrack,
    TrackTooLarge,
    FileNotFound,
}

#[allow(async_fn_in_trait)]
pub trait DiskImage {
    /// Loads a specific half-track into the provided buffer.
    /// Returns the layout of the loaded track.
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error>;
}

#[derive(Clone, Debug)]
pub struct TrackLookupTable {
    pub offsets: [Option<u32>; 86],
    pub lengths: [Option<u16>; 86],
    pub speed_zones: [Option<u8>; 86],
}

impl TrackLookupTable {
    pub fn new() -> Self {
        Self {
            offsets: [None; 86],
            lengths: [None; 86],
            speed_zones: [None; 86],
        }
    }
}

impl Default for TrackLookupTable {
    fn default() -> Self {
        Self::new()
    }
}

#[inline(always)]
pub const fn sectors_per_track(track: u8) -> u8 {
    match track {
        1..=17 => 21,
        18..=24 => 19,
        25..=30 => 18,
        31..=40 => 17,
        _ => 0,
    }
}

pub enum AnyDiskImage<S: Read + Seek, D: DirectorySource> {
    D64(D64Image<S>),
    G64(G64Image<S>),
    VirtualDisk(VirtualDiskImage<S, D>),
}

impl<S: Read + Seek, D: DirectorySource> AnyDiskImage<S, D> {
    pub fn as_virtual_disk_mut(&mut self) -> Option<&mut VirtualDiskImage<S, D>> {
        if let Self::VirtualDisk(img) = self {
            Some(img)
        } else {
            None
        }
    }
}

impl<S: Read + Seek, D: DirectorySource> DiskImage for AnyDiskImage<S, D> {
    async fn read_track(
        &mut self,
        half_track: u8,
        dest: &mut [u8],
    ) -> Result<(TrackLayout, usize, Option<u8>), Error> {
        match self {
            Self::D64(img) => img.read_track(half_track, dest).await,
            Self::G64(img) => img.read_track(half_track, dest).await,
            Self::VirtualDisk(img) => img.read_track(half_track, dest).await,
        }
    }
}

unsafe impl<S: embedded_io_async::Read + embedded_io_async::Seek, D: crate::storage::DirectorySource>
    Send for AnyDiskImage<S, D>
{
}
