use super::{FileOpener, LedsDriver, TrackCacheTask};
use embedded_io_async::{Read, Seek};

impl<
        S: Read + Seek + 'static,
        O: FileOpener<FileStorage = S>,
        H: pico1541_common::interface::TrackHandoff,
        L: LedsDriver,
    > TrackCacheTask<S, O, H, L>
{
    pub(super) async fn handle_timer_event(&mut self, _motor_on: bool) {
        // Static Track 18 directory paging does not require background sector prefetching.
    }
}
