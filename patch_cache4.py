import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

# Replace track_cache_data
content = content.replace("    track_cache_data: [alloc::boxed::Box<[u8]>; 2],", "    track_cache_data: &'static mut [[u8; 8192]; 2],")

# Update DiskTask definition
content = content.replace("pub struct DiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {", "pub struct DiskTask<'a, S: Read + Seek + 'static, D: DirectorySource + 'static, O: FileOpener<FileStorage = S>> {\n    pub track_cache_data_ref: Option<&'static mut [[u8; 8192]; 2]>, // Used to pass the static ref")

# We can't easily change the constructor without knowing exactly what we want.
