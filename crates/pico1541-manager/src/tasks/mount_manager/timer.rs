use super::{FileOpener, TrackCacheTask};
use crate::formats::DiskImage;
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;
use pico1541_common::DriveLocation;

impl<
        S: Read + Seek + 'static,
        O: FileOpener<FileStorage = S>,
        H: pico1541_common::interface::TrackHandoff,
    > TrackCacheTask<S, O, H>
{
    pub(super) async fn handle_timer_event(&mut self, _motor_on: bool) {
        // (first_missing_block, batch_count)
        let mut block_batch: Option<(usize, usize)> = None;
        {
            let mut disk_opt = self.disk_image.as_mut();
            if let Some(disk) = disk_opt.as_mut() {
                if let Some(vdir) = disk.as_virtual_disk_mut() {
                    // Background prefetching for paged directory source
                    if let Some(paged) = vdir.source.as_paged_mut() {
                        // Stop prefetching once the directory end has been discovered.
                        if !paged.is_exhausted() {
                            let current_idx = paged.current_read_index();
                            let current_block = current_idx / 16;
                            let (start_block, end_block) = paged.loaded_range();

                            // Collect consecutive missing blocks in the lookahead window.
                            // One list_dir_range call covers them all in a single forward
                            // scan, avoiding O(N) re-scans from entry 0.
                            let mut batch_start = None;
                            let mut batch_end = 0usize;
                            for offset in 0..3 {
                                let tb = current_block + offset;
                                if tb < start_block || tb >= end_block {
                                    if batch_start.is_none() {
                                        batch_start = Some(tb);
                                        batch_end = tb + 1;
                                    } else if tb == batch_end {
                                        batch_end += 1;
                                    }
                                }
                            }

                            if let Some(bs) = batch_start {
                                block_batch = Some((bs, batch_end - bs));
                            }
                        }
                    }
                }
            }
        }

        if let Some((blocks_start, blocks_count)) = block_batch {
            // Mark active track as Void during background directory fetch to prevent Core 0 emu hang/timeouts.
            let old_layout = self.track_handle.get_active_layout();
            self.track_handle.set_active_layout(TrackLayout::Void);

            let (path, use_internal) = match *self.location.lock().await {
                DriveLocation::VirtualDirectory { ref path, use_internal } => (path.clone(), use_internal),
                _ => return,
            };

            // Fetch all missing blocks in a single sequential pass.
            // Each block holds 16 entries; use a 3×16=48 entry buffer max.
            let fetch_start = blocks_start * 16;
            let fetch_count = blocks_count * 16;
            let mut all_entries = heapless::Vec::<crate::storage::DirectoryEntry, 48>::new();
            let fetch_res = self
                .opener
                .list_dir_range(
                    path.as_str(),
                    use_internal,
                    fetch_start,
                    fetch_count,
                    |entry| {
                        let _ = all_entries.push(entry);
                    },
                )
                .await;

            if fetch_res.is_ok() {
                let mut disk_opt = self.disk_image.as_mut();
                if let Some(disk) = disk_opt.as_mut() {
                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                        if let Some(paged) = vdir.source.as_paged_mut() {
                            // Distribute fetched entries into their correct block slots.
                            // set_block auto-detects end-of-directory from partial blocks.
                            for b in 0..blocks_count {
                                let slice_start = (b * 16).min(all_entries.len());
                                let slice_end = (slice_start + 16).min(all_entries.len());
                                let block_slice = &all_entries[slice_start..slice_end];

                                let mut block_entries =
                                    heapless::Vec::<crate::storage::DirectoryEntry, 16>::new();
                                for e in block_slice {
                                    let _ = block_entries.push(*e);
                                }
                                paged.set_block(blocks_start + b, &block_entries);
                                if block_slice.len() < 16 {
                                    break; // We reached the end of the directory, don't write trailing empty blocks!
                                }
                            }

                            // We fetched new blocks. If the active track is 18, we must regenerate the GCR data!
                            let active_ht = self.track_handle.get_head_half_track();
                            if active_ht == 34 || active_ht == 35 {
                                let data = self.track_handle.start_track_modification();
                                let _ = vdir.read_track(active_ht, data).await;
                                self.track_handle.commit_modified_track();
                            }
                        }
                    }
                }
            } else {
                let mut disk_opt = self.disk_image.as_mut();
                if let Some(disk) = disk_opt.as_mut() {
                    if let Some(vdir) = disk.as_virtual_disk_mut() {
                        if let Some(paged) = vdir.source.as_paged_mut() {
                            let current_blocks = paged.loaded_range().1;
                            paged.set_total_count(current_blocks * 16);
                        }
                    }
                }
            }

            self.track_handle.set_active_layout(old_layout);
        }
    }
}
