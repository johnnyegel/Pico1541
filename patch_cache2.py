import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Replace CachedTrack struct
old_cached = """struct CachedTrack {
    half_track: u8,
    data: alloc::boxed::Box<[u8; 8192]>,
    layout: (pico1541_common::track_buffer::TrackLayout, usize, Option<u8>),
}"""
new_cached = """struct CachedTrack {
    half_track: u8,
    buffer_idx: usize,
    layout: (pico1541_common::track_buffer::TrackLayout, usize, Option<u8>),
}"""
content = content.replace(old_cached, new_cached)

# Replace track_cache field
content = content.replace("    track_cache: alloc::vec::Vec<CachedTrack>,", "    track_cache: heapless::Vec<CachedTrack, 2>,\n    track_cache_data: [alloc::boxed::Box<[u8]>; 2],")

# Replace DiskTask::new init
content = content.replace("            track_cache: alloc::vec::Vec::new(),", "            track_cache: heapless::Vec::new(),\n            track_cache_data: [\n                alloc::vec![0u8; 8192].into_boxed_slice(),\n                alloc::vec![0u8; 8192].into_boxed_slice(),\n            ],")

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)
