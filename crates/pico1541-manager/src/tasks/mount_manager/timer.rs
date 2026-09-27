use super::{FileOpener, TrackCacheTask};
use crate::storage::DirectorySource;
use embedded_io_async::{Read, Seek};
use pico1541_common::track_buffer::TrackLayout;

impl<
        S: Read + Seek + 'static,
        O: FileOpener<FileStorage = S>,
        H: pico1541_common::interface::TrackHandoff,
    > TrackCacheTask<S, O, H>
{
    pub(super) async fn handle_timer_event(&mut self, motor_on: bool) {
        // (first_missing_block, batch_count)
        let mut block_batch: Option<(usize, usize)> = None;
        {
            let mut disk_opt = self.disk_image.as_mut();
            if let Some(disk) = disk_opt.as_mut() {
                if let Some(vdir) = disk.as_virtual_disk_mut() {
                    // Periodic update of directory buffer only when motor is on
                    if motor_on {
                        let current_idx = self.track_handle.get_current_byte_idx();

                        if vdir.file_storage.is_some() {
                            // File streaming treadmill on track 1.
                            // Guard: only update the GCR when the head is on track 1
                            // (half-tracks 0 or 1).  Both track 1 and track 18 now
                            // use Accurate layout, so we can't distinguish by layout —
                            // we must check the physical head position instead.
                            let ht = self.track_handle.get_head_half_track();
                            if ht <= 1 {
                                let data = self.track_handle.start_track_modification();
                                if vdir.update_dynamic_buffer(current_idx, data).await {
                                    self.track_handle.commit_modified_track();
                                }
                            }
                        } else if !vdir
                            .source
                            .as_paged_mut()
                            .map(|p| p.is_exhausted())
                            .unwrap_or(true)
                        {
                            // Track 18: Directory Listing (Accurate(7142))
                            let s1 = vdir.sector1_idx;
                            let s2 = vdir.sector2_idx;

                            if s1 > 0 && s2 > 0 {
                                let s2_end = s2 + 325;
                                let s1_start = s1.saturating_sub(40);

                                // Dead zone between Sector 2 and Sector 1
                                if current_idx > s2_end && current_idx < s1_start {
                                    if vdir.next_sector_to_update == 2 {
                                        // Head has passed Sector 2. Swap to the next batch!
                                        let data = self.track_handle.start_track_modification();
                                        vdir.force_update_directory_sectors(data).await;
                                        self.track_handle.commit_modified_track();

                                        // Prevent further swaps until we pass Sector 2 again
                                        vdir.next_sector_to_update = 1;
                                    }
                                } else if current_idx < s2 {
                                    // Reset the trigger flag when head loops around before Sector 2
                                    vdir.next_sector_to_update = 2;
                                }
                            }
                        }
                    }

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

            let (path, use_internal) = {
                let guard = self.mounted_dir.lock().await;
                (guard.0.clone(), guard.1)
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
                                vdir.next_sector_to_update = 1;
                                let data = self.track_handle.start_track_modification();
                                vdir.force_update_directory_sectors(data).await;
                                self.track_handle.commit_modified_track();
                                vdir.treadmill_spinup_delay = 10;
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
