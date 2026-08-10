import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Replace the inner loop of cache_adjacent_tracks
old_inner = """            let mut new_cache_data = alloc::boxed::Box::new([0u8; 8192]);
            
            let mut disk_opt = self.disk_image.lock().await;
            if let Some(disk) = disk_opt.as_mut() {
                match embassy_futures::select::select(disk.read_track(ht, &mut *new_cache_data), TRACK_CHANNEL.receive()).await {
                    embassy_futures::select::Either::First(Ok(layout)) => {
                        if self.track_cache.len() >= 2 {
                            self.track_cache.remove(0);
                        }
                        self.track_cache.push(CachedTrack {
                            half_track: ht,
                            data: new_cache_data,
                            layout: (layout.0, layout.1, layout.2),
                        });
                        debug!("Pre-cached half-track {}", ht);
                        return None;
                    }
                    embassy_futures::select::Either::First(Err(_)) => {
                        return None;
                    }
                    embassy_futures::select::Either::Second(new_ht) => {
                        return Some(new_ht);
                    }
                }
            }"""

new_inner = """            let mut next_buffer_idx = 0;
            if self.track_cache.len() == 1 {
                next_buffer_idx = 1 - self.track_cache[0].buffer_idx;
            } else if self.track_cache.len() >= 2 {
                let removed = self.track_cache.remove(0);
                next_buffer_idx = removed.buffer_idx;
            }

            let dest_buffer = &mut self.track_cache_data[next_buffer_idx];
            // Safety: dest_buffer is exactly 8192 bytes
            let dest_array: &mut [u8; 8192] = unsafe { &mut *(dest_buffer.as_mut_ptr() as *mut [u8; 8192]) };
            
            let mut disk_opt = self.disk_image.lock().await;
            if let Some(disk) = disk_opt.as_mut() {
                match embassy_futures::select::select(disk.read_track(ht, dest_array), TRACK_CHANNEL.receive()).await {
                    embassy_futures::select::Either::First(Ok(layout)) => {
                        let _ = self.track_cache.push(CachedTrack {
                            half_track: ht,
                            buffer_idx: next_buffer_idx,
                            layout: (layout.0, layout.1, layout.2),
                        });
                        debug!("Pre-cached half-track {}", ht);
                        return None;
                    }
                    embassy_futures::select::Either::First(Err(_)) => {
                        return None;
                    }
                    embassy_futures::select::Either::Second(new_ht) => {
                        return Some(new_ht);
                    }
                }
            }"""

content = content.replace(old_inner, new_inner)

# Replace the retrieval in handle_track_event
old_retrieval = """            if let Some(cached_idx) = self.track_cache.iter().position(|c| c.half_track == target_ht) {
                let cached = &self.track_cache[cached_idx];
                let layout = cached.layout;
                let data = self.track_handle.start_track_load();
                data.copy_from_slice(&*cached.data);"""

new_retrieval = """            if let Some(cached_idx) = self.track_cache.iter().position(|c| c.half_track == target_ht) {
                let cached = &self.track_cache[cached_idx];
                let layout = cached.layout;
                let buffer_idx = cached.buffer_idx;
                let data = self.track_handle.start_track_load();
                data.copy_from_slice(&*self.track_cache_data[buffer_idx]);"""

content = content.replace(old_retrieval, new_retrieval)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
