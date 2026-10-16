use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::Vec;
use pico1541_manager::driver::DisplayDriver;
use pico1541_manager::tasks::menu::{Menu, MenuEvent};
use pico1541_manager::{UiAction, LOCATION_CHANGED};

#[no_mangle]
fn _embassy_time_now() -> u64 {
    0
}

#[no_mangle]
fn _embassy_time_schedule_wake(_at: u64, _waker: &core::task::Waker) {}

struct MockDisplay;

impl DisplayDriver for MockDisplay {
    async fn clear(&mut self) {}
    async fn print(&mut self, _line: u8, _text: &str) {}
    async fn print_large(&mut self, _line: u8, _text: &str) {}
    async fn update(&mut self) {}
    async fn redraw(&mut self) {}
    fn request_redraw(&mut self) {}
    fn is_dirty(&self) -> bool {
        false
    }
}

#[test]
fn test_menu_wakes_on_location_changed() {
    let mut items = Vec::<&'static str, 4>::new();
    let _ = items.push("Item 1");
    let _ = items.push("Item 2");
    let mut menu = Menu::new("Test Menu", items);

    let channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let mut display = MockDisplay;

    // Signal location changed before run
    LOCATION_CHANGED.signal(());

    let event = block_on(menu.run(&mut display, &channel.receiver()));
    match event {
        MenuEvent::LocationChanged => {}
        _ => panic!("Expected MenuEvent::LocationChanged, got other event"),
    }
}

#[test]
fn test_menu_receives_ui_action() {
    let mut items = Vec::<&'static str, 4>::new();
    let _ = items.push("Item 1");
    let _ = items.push("Item 2");
    let mut menu = Menu::new("Test Menu", items);

    let channel = Box::leak(Box::new(Channel::<CriticalSectionRawMutex, UiAction, 16>::new()));
    let mut display = MockDisplay;

    // Send select action
    let _ = channel.sender().try_send(UiAction::Select);

    let event = block_on(menu.run(&mut display, &channel.receiver()));
    match event {
        MenuEvent::Selected(idx) => assert_eq!(idx, 0),
        _ => panic!("Expected MenuEvent::Selected(0)"),
    }
}

#[test]
fn test_is_mounted_case_insensitive() {
    use pico1541_manager::tasks::ui::browser::is_mounted;
    use heapless::String;

    let mut mounted = String::<128>::new();
    let _ = mounted.push_str("/demos/DISK1.D64");

    // Exact match
    assert!(is_mounted("/demos/DISK1.D64", Some(&mounted)));
    // Lowercase match
    assert!(is_mounted("/demos/disk1.d64", Some(&mounted)));
    // Mixed case match
    assert!(is_mounted("/Demos/Disk1.D64", Some(&mounted)));
    // Different path
    assert!(!is_mounted("/demos/disk2.d64", Some(&mounted)));
    // None mounted
    assert!(!is_mounted("/demos/disk1.d64", None));
}

#[test]
fn test_format_menu_block_directories_and_mounted() {
    use pico1541_manager::driver::FileInfo;
    use pico1541_manager::tasks::ui::browser::format_menu_block;
    use heapless::String;

    let mut dir_info = FileInfo {
        name: [0u8; 64],
        size: 0,
        is_dir: true,
    };
    dir_info.name[..5].copy_from_slice(b"demos");

    let mut file1_info = FileInfo {
        name: [0u8; 64],
        size: 1000,
        is_dir: false,
    };
    file1_info.name[..9].copy_from_slice(b"game1.d64");

    let mut file2_info = FileInfo {
        name: [0u8; 64],
        size: 2000,
        is_dir: false,
    };
    file2_info.name[..9].copy_from_slice(b"game2.d64");

    let infos = [dir_info, file1_info, file2_info];

    let mut mounted_path = String::<128>::new();
    let _ = mounted_path.push_str("/files/game1.d64");

    let formatted = format_menu_block(&infos, "/files", Some(&mounted_path));
    assert_eq!(formatted.len(), 3);
    assert_eq!(formatted[0].as_str(), "demos/");
    assert_eq!(formatted[1].as_str(), "*game1.d64");
    assert_eq!(formatted[2].as_str(), "game2.d64");
}

static VDIR_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct MockStorage {
    entries: std::vec::Vec<pico1541_manager::driver::FileInfo>,
    list_dir_called: std::rc::Rc<core::cell::Cell<bool>>,
}

impl MockStorage {
    fn new(
        entries: std::vec::Vec<pico1541_manager::driver::FileInfo>,
        list_dir_called: std::rc::Rc<core::cell::Cell<bool>>,
    ) -> Self {
        Self {
            entries,
            list_dir_called,
        }
    }
}

impl pico1541_manager::driver::FileStorageDriver for MockStorage {
    type File = pico1541_manager::storage::MemoryStorage<'static>;

    async fn is_available(&self) -> bool {
        true
    }

    async fn list_dir(
        &mut self,
        _path: &str,
        mut callback: impl FnMut(pico1541_manager::driver::FileInfo),
    ) -> Result<(), pico1541_manager::formats::Error> {
        self.list_dir_called.set(true);
        for entry in &self.entries {
            callback(*entry);
        }
        Ok(())
    }

    async fn open_file(&mut self, _path: &str) -> Result<(), pico1541_manager::formats::Error> {
        Ok(())
    }

    fn open_file_handle(&self, _path: &str) -> Self::File {
        pico1541_manager::storage::MemoryStorage::new(&[])
    }
}

#[test]
fn test_virtual_directory_manager_active_children_priority() {
    use pico1541_manager::tasks::mount_manager::{CachedChildEntry, ACTIVE_CHILDREN};
    use pico1541_manager::tasks::ui::virtual_directory::VirtualDirectoryManager;
    use heapless::String;

    let _lock = VDIR_TEST_MUTEX.lock().unwrap();

    let called = std::rc::Rc::new(core::cell::Cell::new(false));
    let mut storage = MockStorage::new(std::vec![], called.clone());
    let mut internal = MockStorage::new(std::vec![], std::rc::Rc::new(core::cell::Cell::new(false)));
    let mut current_path = String::<128>::new();
    let _ = current_path.push_str("/files");

    block_on(async {
        {
            let mut guard = ACTIVE_CHILDREN.lock().await;
            guard.clear();
            let mut s1 = String::new();
            let _ = s1.push_str("demo.prg");
            guard.push(CachedChildEntry {
                petscii_name: [0x41; 16],
                real_name: s1,
                is_dir: false,
            }).unwrap();

            let mut s2 = String::new();
            let _ = s2.push_str("subfolder");
            guard.push(CachedChildEntry {
                petscii_name: [0x42; 16],
                real_name: s2,
                is_dir: true,
            }).unwrap();
        }

        let mut vdir_mgr = VirtualDirectoryManager {
            use_internal: false,
            storage: &mut storage,
            internal: &mut internal,
            current_path: &mut current_path,
        };

        let block = vdir_mgr.ensure_block_loaded(0).await.unwrap();
        assert_eq!(block.len(), 2);
        assert!(!block[0].is_dir);
        assert!(block[1].is_dir);

        let entry0 = vdir_mgr.fetch_entry_at(0).await.unwrap();
        assert_eq!(core::str::from_utf8(&entry0.name).unwrap().trim_matches('\0'), "demo.prg");
        assert!(!entry0.is_dir);

        let entry1 = vdir_mgr.fetch_entry_at(1).await.unwrap();
        assert_eq!(core::str::from_utf8(&entry1.name).unwrap().trim_matches('\0'), "subfolder");
        assert!(entry1.is_dir);

        // Out of bounds entry with active children should return None without calling fallback storage
        let entry_oob = vdir_mgr.fetch_entry_at(2).await;
        assert!(entry_oob.is_none());
        assert!(!called.get());

        {
            let mut guard = ACTIVE_CHILDREN.lock().await;
            guard.clear();
        }
    });
}

#[test]
fn test_virtual_directory_manager_fallback_to_storage() {
    use pico1541_manager::driver::FileInfo;
    use pico1541_manager::tasks::mount_manager::ACTIVE_CHILDREN;
    use pico1541_manager::tasks::ui::virtual_directory::VirtualDirectoryManager;
    use heapless::String;

    let _lock = VDIR_TEST_MUTEX.lock().unwrap();

    let mut file_info = FileInfo {
        name: [0u8; 64],
        size: 512,
        is_dir: false,
    };
    file_info.name[..8].copy_from_slice(b"disk.d64");

    let called = std::rc::Rc::new(core::cell::Cell::new(false));
    let mut storage = MockStorage::new(std::vec![file_info], called.clone());
    let mut internal = MockStorage::new(std::vec![], std::rc::Rc::new(core::cell::Cell::new(false)));
    let mut current_path = String::<128>::new();
    let _ = current_path.push_str("/");

    block_on(async {
        {
            let mut guard = ACTIVE_CHILDREN.lock().await;
            guard.clear();
        }

        let mut vdir_mgr = VirtualDirectoryManager {
            use_internal: false,
            storage: &mut storage,
            internal: &mut internal,
            current_path: &mut current_path,
        };

        let block = vdir_mgr.ensure_block_loaded(0).await.unwrap();
        assert_eq!(block.len(), 1);
        assert_eq!(core::str::from_utf8(&block[0].name).unwrap().trim_matches('\0'), "disk.d64");
        assert!(called.get());

        let entry = vdir_mgr.fetch_entry_at(0).await.unwrap();
        assert_eq!(core::str::from_utf8(&entry.name).unwrap().trim_matches('\0'), "disk.d64");
    });
}

