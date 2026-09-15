use crate::driver::{FileInfo, FileStorageDriver};
use crate::storage::DirectoryEntry;
use heapless::{String, Vec};

pub struct VirtualDirectoryManager<'a, S, Internal>
where
    S: FileStorageDriver,
    Internal: FileStorageDriver,
{
    pub use_internal: bool,
    pub storage: &'a mut S,
    pub internal: &'a mut Internal,
    pub current_path: &'a mut String<128>,
}

impl<'a, S, Internal> VirtualDirectoryManager<'a, S, Internal>
where
    S: FileStorageDriver,
    Internal: FileStorageDriver,
{
    pub async fn mount_virtual_directory(&mut self, dir_name: &str) {
        let mut path: heapless::String<128> = heapless::String::new();
        let _ = path.push_str(dir_name);
        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::Mount(
            crate::MountPayload::VirtualDirectory(path, self.use_internal),
        ));
    }

    pub async fn ensure_block_loaded(&mut self, block_idx: usize) -> Result<Vec<FileInfo, 16>, ()> {
        let mut block_infos = Vec::<FileInfo, 16>::new();
        let path_str = self.current_path.as_str();

        let fetch_res = if self.use_internal {
            self.internal
                .list_dir_range(path_str, block_idx * 16, 16, |info| {
                    let _ = block_infos.push(info);
                })
                .await
        } else {
            self.storage
                .list_dir_range(path_str, block_idx * 16, 16, |info| {
                    let _ = block_infos.push(info);
                })
                .await
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
            let entry =
                DirectoryEntry::from_file_info(&info.name[..name_len], info.is_dir, info.size);
            let _ = entries.push(entry);
        }

        Ok(block_infos)
    }
}
