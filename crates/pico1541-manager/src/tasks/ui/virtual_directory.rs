use crate::driver::{FileInfo, FileStorageDriver};
use crate::storage::PAGE_CAPACITY;
use crate::tasks::mount_manager::{ACTIVE_CHILDREN, CURRENT_PAGE};
use crate::tasks::ui::UiContext;
use core::sync::atomic::Ordering;
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
    pub context: &'a UiContext,
}

impl<'a, S, Internal> VirtualDirectoryManager<'a, S, Internal>
where
    S: FileStorageDriver,
    Internal: FileStorageDriver,
{
    pub async fn ensure_block_loaded(&mut self, block_idx: usize) -> Result<Vec<FileInfo, 16>, ()> {
        let mut block_infos = Vec::<FileInfo, 16>::new();

        {
            let guard = ACTIVE_CHILDREN.lock().await;
            if !guard.is_empty() {
                let start_idx = block_idx * 16;
                if start_idx < guard.len() {
                    let end_idx = (start_idx + 16).min(guard.len());
                    for entry in &guard[start_idx..end_idx] {
                        let _ = block_infos.push(entry.to_file_info());
                    }
                }
                return Ok(block_infos);
            }
        }

        let path_str = self.current_path.as_str();
        let page_offset = CURRENT_PAGE.load(Ordering::Acquire) * PAGE_CAPACITY;
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

    pub async fn fetch_entry_at(&mut self, idx: usize) -> Option<FileInfo> {
        {
            let guard = ACTIVE_CHILDREN.lock().await;
            if idx < guard.len() {
                return Some(guard[idx].to_file_info());
            }
        }

        let page_offset = CURRENT_PAGE.load(Ordering::Acquire) * PAGE_CAPACITY;
        let start_entry = page_offset + idx;
        let path_str = self.current_path.as_str();

        let mut entry_info = None;
        let fetch_res = if self.use_internal {
            self.internal
                .list_dir_range(path_str, start_entry, 1, |info| {
                    entry_info = Some(info);
                })
                .await
        } else {
            self.storage
                .list_dir_range(path_str, start_entry, 1, |info| {
                    entry_info = Some(info);
                })
                .await
        };

        if fetch_res.is_err() {
            return None;
        }

        entry_info
    }
}
