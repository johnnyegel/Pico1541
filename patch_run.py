import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

new_block = """                        self.track_handle.set_active_layout(old_layout);
                    }
                    
                    if let Some(new_ht) = self.cache_adjacent_tracks().await {
                        self.handle_track_event(new_ht).await;
                    }
                }
                Either3::Third(cmd) => {"""

content = content.replace("                        self.track_handle.set_active_layout(old_layout);\n                    }\n                }\n                Either3::Third(cmd) => {", new_block)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
