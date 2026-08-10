import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Insert cache_adjacent_tracks
new_method = """
    async fn cache_adjacent_tracks(&mut self) -> Option<u8> {
        let current_ht = self.track_handle.get_head_half_track();
        
        let desired_tracks = [
            current_ht.saturating_add(2),
            current_ht.saturating_sub(2),
        ];

        for &ht in &desired_tracks {
            if ht == 0 || ht > 84 {
                continue;
            }
            if ht == current_ht {
                continue;
            }
            if self.track_cache.iter().any(|c| c.half_track == ht) {
                continue;
            }

            let mut new_cache_data = alloc::boxed::Box::new([0u8; pico1541_common::track_buffer::TRACK_SIZE]);
            
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
            }
        }
        None
    }

    async fn handle_track_event(&mut self, half_track: u8) {"""

content = content.replace("    async fn handle_track_event(&mut self, half_track: u8) {", new_method)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
