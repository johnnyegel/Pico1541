use super::{FileOpener, TrackCacheTask};
use crate::formats::DiskImage;
use embassy_futures::select::select;
use embassy_futures::select::Either;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;

#[cfg(feature = "defmt")]
use defmt::{debug, error, info};
#[cfg(not(feature = "defmt"))]
use log::{debug, error, info};

impl<
        S: Read + Seek + 'static,
        O: FileOpener<FileStorage = S>,
        H: pico1541_common::interface::TrackHandoff,
    > TrackCacheTask<S, O, H>
{
        pub(super) async fn handle_track_event(&mut self, half_track: u8) {
        let mut target_ht = half_track;

        let mut loop_cnt = 0;
        loop {
            loop_cnt += 1;
            if loop_cnt > 10 {
                #[cfg(feature = "defmt")]
                defmt::error!("handle_track_event stuck in infinite loop!");
            }
            let requested = self.handoff.current_track();
            if requested != target_ht {
                target_ht = requested;
            }
            debug!(
                "Manager loading Track {}.{} (HT: {})",
                (target_ht / 2) + 1,
                target_ht % 2,
                target_ht
            );
            // Prevent DOS from reading the old track data by stalling it safely.
            let old_layout = self.track_handle.get_active_layout();
            self.track_handle.set_active_layout(TrackLayout::Void);

            let mut disk_opt = self.disk_image.as_mut();
            if let Some(disk) = disk_opt.as_mut() {
                let data = self.track_handle.start_track_load();

                let start_time = embassy_time::Instant::now();
                // Read track but CANCEL if the head steps again!
                match select(
                    disk.read_track(target_ht, data),
                    self.handoff.wait_for_change(),
                )
                .await
                {
                    Either::First(Ok(layout)) => {
                        let speed_zone = layout.2.unwrap_or_else(|| {
                            pico1541_common::track_buffer::default_speed_zone(target_ht)
                        });
                        let identifier = pico1541_common::track_buffer::TrackIdentifier::new(
                            target_ht, speed_zone,
                        );

                        if layout.0 == TrackLayout::Void {
                            // If the track is completely empty, there's no need to commit the back-buffer
                            // (which swaps buffers and pads it with 0x55). We just update the active identifier!
                            self.track_handle.set_active_track_identifier(identifier);
                            
                        } else {
                            self.track_handle
                                .commit_loaded_track(identifier, layout.0, layout.1);
                        }

                        let elapsed = start_time.elapsed().as_millis();
                        info!(
                            "Track {}.{} (HT: {}) load took {} ms",
                            (target_ht / 2) + 1,
                            target_ht % 2,
                            target_ht,
                            elapsed
                        );
                        break; // Success, done loading the track!
                    }
                    Either::First(Err(e)) => {
                        error!("Failed to read track: {:?}", &e);
                        self.track_handle.set_active_layout(old_layout);
                        break;
                    }
                    Either::Second(_) => {
                        // The head stepped WHILE we were reading from the SD card!
                        // The read is aborted. We loop around to start reading the NEW track!
                        target_ht = self.handoff.current_track();
                        continue;
                    }
                }
            } else {
                self.track_handle.commit_loaded_track(
                    pico1541_common::track_buffer::TrackIdentifier::new(
                        target_ht,
                        pico1541_common::track_buffer::default_speed_zone(target_ht),
                    ),
                    TrackLayout::Void,
                    0,
                );
                info!("Half-Track {} is void...", target_ht);
                break;
            }
        }
    }
}
