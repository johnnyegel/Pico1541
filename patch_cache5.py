import re

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "r") as f:
    content = f.read()

content = content.replace("    track_cache_data: [alloc::boxed::Box<[u8]>; 2],", "    track_cache_data: &'static mut [[u8; 8192]; 2],")

old_new = """    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        track_handle: ManagerHandle,
        opener: O,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    ) -> Self {
        Self {
            disk_image,
            track_handle,
            opener,
            mounted_dir,
            track_cache: heapless::Vec::new(),
            track_cache_data: [
                alloc::vec![0u8; 8192].into_boxed_slice(),
                alloc::vec![0u8; 8192].into_boxed_slice(),
            ],
        }"""

new_new = """    pub fn new(
        disk_image: &'a Mutex<CriticalSectionRawMutex, Option<AnyDiskImage<S, D>>>,
        track_handle: ManagerHandle,
        opener: O,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        track_cache_data: &'static mut [[u8; 8192]; 2],
    ) -> Self {
        Self {
            disk_image,
            track_handle,
            opener,
            mounted_dir,
            track_cache: heapless::Vec::new(),
            track_cache_data,
        }"""

content = content.replace(old_new, new_new)

old_dest = """            let dest_buffer = &mut self.track_cache_data[next_buffer_idx];
            // Safety: dest_buffer is exactly 8192 bytes
            let dest_array: &mut [u8; 8192] = unsafe { &mut *(dest_buffer.as_mut_ptr() as *mut [u8; 8192]) };"""

new_dest = """            let dest_array = &mut self.track_cache_data[next_buffer_idx];"""

content = content.replace(old_dest, new_dest)

with open("crates/pico1541-manager/src/tasks/disk_task.rs", "w") as f:
    f.write(content)

