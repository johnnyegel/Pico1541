import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Update handle_track_event
new_handle = """
            debug!("Manager loading half-track: {}", target_ht);

            if let Some(cached_idx) = self.track_cache.iter().position(|c| c.half_track == target_ht) {
                let cached = &self.track_cache[cached_idx];
                let layout = cached.layout;
                let data = self.track_handle.start_track_load();
                data.copy_from_slice(&*cached.data);
                self.track_handle.commit_loaded_track(
                    pico1541_common::track_buffer::TrackIdentifier::new(target_ht, layout.2.unwrap_or_else(|| pico1541_common::track_buffer::default_speed_zone(target_ht))),
                    layout.0,
                    layout.1
                );
                info!("Half-Track {} load took 0 ms (CACHED)", target_ht);
                break;
            }

            // Prevent DOS from reading the old track data by stalling it safely.
"""

content = content.replace('            debug!("Manager loading half-track: {}", target_ht);\n\n            // Prevent DOS from reading the old track data by stalling it safely.\n', new_handle)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
