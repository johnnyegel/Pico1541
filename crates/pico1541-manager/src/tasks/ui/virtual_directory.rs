use heapless::{String, Vec};
use crate::driver::{FileStorageDriver, FileInfo};
use crate::storage::{DirectoryEntry, DirectorySource};
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub struct VirtualDirectoryManager<'a, S, Internal, File>
where
    S: FileStorageDriver<File = File>,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    pub use_internal: bool,
    pub storage: &'a mut S,
    pub internal: &'a mut Internal,
    pub current_path: &'a mut String<128>,
    pub disk_image_mutex: &'a Mutex<CriticalSectionRawMutex, Option<crate::AnyDiskImage<crate::storage::RuntimeStorage<'static, File, Internal::File>, crate::storage::PagedDirectorySource<144>>>>,
}

impl<'a, S, Internal, File> VirtualDirectoryManager<'a, S, Internal, File>
where
    S: FileStorageDriver<File = File>,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    pub async fn mount_virtual_directory(&mut self, dir_name: &str) {
        let dir_source = crate::storage::PagedDirectorySource::<144>::new(dir_name.as_bytes());
        let vdisk = crate::formats::VirtualDiskImage::new(dir_source, dir_name.as_bytes());
        
        let mut disk_lock = self.disk_image_mutex.lock().await;
        *disk_lock = Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk));
        
        let mut default_name: heapless::String<128> = heapless::String::new();
        let _ = default_name.push_str("Internal Virtual Disk");
        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted(default_name));
    }

    pub async fn ensure_block_loaded(&mut self, block_idx: usize) -> Result<Vec<FileInfo, 16>, ()> {
        let mut block_infos = Vec::<FileInfo, 16>::new();
        let path_str = self.current_path.as_str();
        
        let fetch_res = if self.use_internal {
            self.internal.list_dir_range(path_str, block_idx * 16, 16, |info| {
                let _ = block_infos.push(info);
            }).await
        } else {
            self.storage.list_dir_range(path_str, block_idx * 16, 16, |info| {
                let _ = block_infos.push(info);
            }).await
        };

        if fetch_res.is_err() {
            return Err(());
        }

        let mut entries = Vec::<DirectoryEntry, 16>::new();
        for info in block_infos.iter() {
            let mut name_len = 0;
            while name_len < info.name.len() && info.name[name_len] != 0 {
                name_len += 1;
            }
            let entry = DirectoryEntry::from_file_info(&info.name[..name_len], info.is_dir, info.size);
            let _ = entries.push(entry);
        }

        // Update the virtual disk cache so the host (C64) can see the new block on Track 18
        if let Some(crate::formats::AnyDiskImage::VirtualDisk(vdisk)) = self.disk_image_mutex.lock().await.as_mut() {
            if let Some(paged) = vdisk.source.as_paged_mut() {
                paged.set_block(block_idx, &entries);
            }
        }

        Ok(block_infos)
    }
}
