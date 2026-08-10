import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

content = "extern crate alloc;\n" + content
content = content.replace("pico1541_common::track_buffer::TRACK_SIZE", "8192")

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
