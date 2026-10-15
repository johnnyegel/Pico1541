use crate::driver::{FileInfo, FileStorageDriver};
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
    pub context: &'a crate::tasks::ui::UiContext,
}

impl<'a, S, Internal> VirtualDirectoryManager<'a, S, Internal>
where
    S: FileStorageDriver,
    Internal: FileStorageDriver,
{
    pub async fn ensure_block_loaded(&mut self, block_idx: usize) -> Result<Vec<FileInfo, 16>, ()> {
        let mut block_infos = Vec::<FileInfo, 16>::new();

        {
            let guard = crate::tasks::mount_manager::ACTIVE_CHILDREN.lock().await;
            if !guard.is_empty() {
                let start_idx = block_idx * 16;
                if start_idx < guard.len() {
                    let end_idx = (start_idx + 16).min(guard.len());
                    for entry in &guard[start_idx..end_idx] {
                        let mut name = [0u8; 64];
                        let nb = entry.real_name.as_bytes();
                        let len = nb.len().min(63);
                        name[..len].copy_from_slice(&nb[..len]);
                        let _ = block_infos.push(FileInfo {
                            name,
                            size: 0,
                            is_dir: entry.is_dir,
                        });
                    }
                }
                return Ok(block_infos);
            }
        }

        let path_str = self.current_path.as_str();
        let page_offset = crate::tasks::mount_manager::CURRENT_PAGE.load(core::sync::atomic::Ordering::Acquire)
            * crate::storage::PAGE_CAPACITY;
        let start_entry = page_offset + block_idx * 16;

        let fetch_res = if self.use_internal {
            self.internal
                .list_dir_range(path_str, start_entry, 16, |info| {
                    let _ = block_infos.push(info);
                })
                .await
        } else {
            self.storage
                .list_dir_range(path_str, start_entry, 16, |info| {
                    let _ = block_infos.push(info);
                })
                .await
        };

        if fetch_res.is_err() {
            return Err(());
        }

        Ok(block_infos)
    }
}
