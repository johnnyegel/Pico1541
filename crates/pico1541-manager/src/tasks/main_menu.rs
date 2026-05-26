use crate::driver::{DisplayDriver, RgbLedDriver, StatusLedMode, IndicatorDriver, FileStorageDriver, FileInfo, InternalStorage};
use crate::tasks::menu::{Menu, MenuEvent};
use crate::tasks::hardware_test::HardwareTestTask;
use crate::{UiAction, ACTION_CHANNEL};
use embassy_time::{Duration, Timer};
use heapless::{Vec, String};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use core::str::FromStr;
use pico1541_common::snooper::SnooperConfig;
use pico1541_common::storage::PagedDirectorySourceTrait;
use portable_atomic::Ordering;

fn parent_dir(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    let p = if path.ends_with('/') && path.len() > 1 {
        &path[..path.len() - 1]
    } else {
        path
    };
    if let Some(idx) = p.rfind('/') {
        if idx == 0 {
            "/"
        } else {
            &p[..idx]
        }
    } else {
        "/"
    }
}

fn join_path(current: &str, sub: &str) -> String<128> {
    let mut new_path = String::<128>::new();
    if current == "/" {
        let _ = new_path.push_str("/");
        let _ = new_path.push_str(sub);
    } else {
        let _ = new_path.push_str(current);
        let _ = new_path.push_str("/");
        let _ = new_path.push_str(sub);
    }
    new_path
}

pub struct MainMenuTask<D, L, Ind, S, File>
where
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    display: D,
    leds: L,
    indicator: Ind,
    storage: S,
    internal: InternalStorage,
    _disk_image: &'static Mutex<CriticalSectionRawMutex, Option<pico1541_common::AnyDiskImage<pico1541_common::storage::RuntimeStorage<'static, File>, pico1541_common::storage::PagedDirectorySource<48>>>>,
    current_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    config: &'static SnooperConfig,
    sd_path: String<128>,
    internal_path: String<128>,
    usb_path: String<128>,
    mounted_file_path: Option<String<128>>,
}

impl<D, L, Ind, S, File> MainMenuTask<D, L, Ind, S, File>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver<File = File> + Clone,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    pub fn new(
        display: D,
        leds: L,
        indicator: Ind,
        storage: S,
        disk_image: &'static Mutex<CriticalSectionRawMutex, Option<pico1541_common::AnyDiskImage<pico1541_common::storage::RuntimeStorage<'static, File>, pico1541_common::storage::PagedDirectorySource<48>>>>,
        current_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        config: &'static SnooperConfig,
    ) -> Self {
        Self {
            display,
            leds,
            indicator,
            storage,
            internal: InternalStorage,
            _disk_image: disk_image,
            current_dir,
            config,
            sd_path: String::from_str("/").unwrap(),
            internal_path: String::from_str("/").unwrap(),
            usb_path: String::from_str("/").unwrap(),
            mounted_file_path: None,
        }
    }

    fn create_directory_entry(&self, name: &[u8], is_dir: bool, size: u32) -> pico1541_common::storage::DirectoryEntry {
        let mut filename = [0xA0; 16];
        let mut len = 0;
        for &b in name.iter() {
            if b == 0 || len >= 16 {
                break;
            }
            let c = if b >= b'a' && b <= b'z' {
                b - b'a' + b'A'
            } else {
                b
            };
            filename[len] = c;
            len += 1;
        }
        
        let file_type = if is_dir {
            0x81
        } else {
            0x82
        };
        
        pico1541_common::storage::DirectoryEntry {
            file_type,
            filename,
            first_track: 1,
            first_sector: 0,
            block_size: ((size + 253) / 254) as u16,
        }
    }

    fn internal_menu_file_data(&self, filename: &str) -> Option<&'static crate::driver::internal_storage::InternalFile> {
        for file in crate::driver::internal_storage::INTERNAL_FILES {
            if file.name.eq_ignore_ascii_case(filename) {
                return Some(file);
            }
        }
        None
    }

    pub async fn run(&mut self) -> ! {
        // Initialize the status LED to the default startup mode (Unmounted - Red)
        self.leds.set_status(StatusLedMode::Unmounted);
        {
            let mut disk = self._disk_image.lock().await;
            *disk = None;
        }

        // 1. Boot Screen
        self.display.clear().await;
        self.display.print_large(2, "  Pico1541").await; 
        self.display.request_redraw();
        Timer::after(Duration::from_millis(1500)).await;

        let mut cursor = 0;
        loop {
            let mut main_items = Vec::<String<16>, 4>::new();

            // 0. Internal
            let mut int_str = String::<16>::from_str("Internal").unwrap();
            let int_available = self.internal.is_available().await;
            if !int_available {
                let _ = int_str.push_str(" (X)");
            }
            let _ = main_items.push(int_str);

            // 1. SD Card
            let mut sd_str = String::<16>::from_str("SD Card").unwrap();
            let sd_available = self.storage.is_available().await;
            if !sd_available {
                let _ = sd_str.push_str(" (X)");
            }
            let _ = main_items.push(sd_str);

            // 2. USB Drive (Not yet supported)
            let mut usb_str = String::<16>::from_str("USB Drive").unwrap();
            let _ = usb_str.push_str(" (X)");
            let _ = main_items.push(usb_str);

            // 3. Settings
            let _ = main_items.push(String::from_str("Settings").unwrap());
            
            let mut main_menu = Menu::new("--- Main Menu ---", main_items);
            main_menu.set_cursor(cursor);

            match main_menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    cursor = idx;
                    match idx {
                        0 => {
                            if int_available {
                                let _ = self.internal_menu().await;
                            }
                        }
                        1 => {
                            if sd_available {
                                let _ = self.sd_card_menu().await;
                            }
                        }
                        3 => {
                            let _ = self.settings_menu().await;
                        }
                        _ => {
                            log::info!("Selected: {:?}", main_menu.selected_item());
                        }
                    }
                }
                MenuEvent::Back => {
                    // Back in main menu could maybe show a "Shutting down" or just ignore
                }
                MenuEvent::Home => {
                    // Home in main menu does nothing
                }
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => {
                    cursor = main_menu.selected_index();
                    if !self.storage.is_available().await {
                        self.sd_path = String::from_str("/").unwrap();
                    }
                    // Just let the loop run again to refresh status icons
                    continue;
                }
            }
        }
    }

    async fn sd_card_menu(&mut self) -> bool {
        self.file_browser_menu("SD Card", false).await
    }

    async fn internal_menu(&mut self) -> bool {
        self.file_browser_menu("Internal", true).await
    }

    async fn file_browser_menu(&mut self, title: &str, use_internal: bool) -> bool {
        let mut cursor: usize = 0;
        let mut scroll_offset: usize = 0;
        let mut cursor_stack = heapless::Vec::<(usize, usize), 32>::new();
        loop {
            let current_path = if use_internal {
                &self.internal_path
            } else if title == "SD Card" {
                &self.sd_path
            } else {
                &self.usb_path
            };

            let path_str = current_path.as_str();
            let mut dir_name_buf = String::<32>::new();
            let dir_name = if path_str == "/" {
                if use_internal {
                    "INTERNAL"
                } else if let Some(label) = self.storage.volume_label() {
                    let _ = dir_name_buf.push_str(label.as_str());
                    dir_name_buf.as_str()
                } else {
                    "SD CARD"
                }
            } else {
                if let Some(idx) = path_str.rfind('/') {
                    &path_str[idx + 1..]
                } else {
                    path_str
                }
            };

            let current_block = cursor / 16;
            let initial_start_block = current_block.saturating_sub(1);
            let initial_start_idx = initial_start_block * 16;

            // Load the entries around the cursor position (up to 48 entries = 3 blocks)
            let mut initial_infos = Vec::<FileInfo, 48>::new();
            let load_res = if use_internal {
                self.internal.list_dir_range(current_path.as_str(), initial_start_idx, 48, |info| {
                    let _ = initial_infos.push(info);
                }).await
            } else {
                self.storage.list_dir_range(current_path.as_str(), initial_start_idx, 48, |info| {
                    let _ = initial_infos.push(info);
                }).await
            };

            if let Err(e) = load_res {
                log::error!("Error listing dir: {:?}", e);
                let current_path_mut = if use_internal {
                    &mut self.internal_path
                } else if title == "SD Card" {
                    &mut self.sd_path
                } else {
                    &mut self.usb_path
                };
                *current_path_mut = String::from_str("/").unwrap();

                self.display.clear().await;
                self.display.print(1, "  Read Error!").await;
                self.display.request_redraw();
                Timer::after(Duration::from_secs(2)).await;
                if !use_internal {
                    let mut disk = self._disk_image.lock().await;
                    *disk = None;
                    self.leds.set_status(StatusLedMode::Unmounted);
                }
                return false;
            }

            // Build directory entries for the virtual disk using PagedDirectorySource.
            // Distribute the fetched entries into blocks; set_block auto-detects end-of-dir.
            let mut dir_source = pico1541_common::storage::PagedDirectorySource::new(dir_name.as_bytes());
            for (block_idx, chunk) in initial_infos.chunks(16).enumerate() {
                let mut block_entries = Vec::<pico1541_common::storage::DirectoryEntry, 16>::new();
                for info in chunk {
                    let mut name_len = 0;
                    while name_len < info.name.len() && info.name[name_len] != 0 {
                        name_len += 1;
                    }
                    let entry = self.create_directory_entry(&info.name[..name_len], info.is_dir, info.size);
                    let _ = block_entries.push(entry);
                }
                dir_source.set_block(initial_start_block + block_idx, &block_entries);
            }

            let vdisk = pico1541_common::formats::VirtualDiskImage::new(dir_source);

            if self.mounted_file_path.is_none() {
                {
                    let mut disk_lock = self._disk_image.lock().await;
                    *disk_lock = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                }
                self.leds.set_status(StatusLedMode::VirtualDisk);
            }

            {
                let mut current_dir_guard = self.current_dir.lock().await;
                current_dir_guard.0 = current_path.clone();
                current_dir_guard.1 = use_internal;
            }

            let mut menu_title = String::<32>::new();
            if path_str == "/" && !use_internal {
                if let Some(label) = self.storage.volume_label() {
                    let _ = core::fmt::write(&mut menu_title, format_args!("SD: {}", label.as_str()));
                } else {
                    let _ = menu_title.push_str("SD CARD");
                }
            } else {
                let _ = menu_title.push_str(dir_name);
            }

            if initial_infos.is_empty() {
                let mut empty_items = Vec::<&'static str, 1>::new();
                let _ = empty_items.push("No files found");
                let mut menu = Menu::new(menu_title, empty_items);
                match menu.run(&mut self.display).await {
                    MenuEvent::Back => {
                        let current_path_mut = if use_internal {
                            &mut self.internal_path
                        } else if title == "SD Card" {
                            &mut self.sd_path
                        } else {
                            &mut self.usb_path
                        };
                        if current_path_mut.as_str() == "/" {
                            let mut disk = self._disk_image.lock().await;
                            *disk = None;
                            self.leds.set_status(StatusLedMode::Unmounted);
                            return false;
                        } else {
                            let parent = parent_dir(current_path_mut.as_str());
                            *current_path_mut = String::from_str(parent).unwrap();
                            let state = cursor_stack.pop().unwrap_or((0, 0));
                            cursor = state.0;
                            scroll_offset = state.1;
                            continue;
                        }
                    }
                    MenuEvent::Home => {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        self.leds.set_status(StatusLedMode::Unmounted);
                        return true;
                    }
                    _ => {}
                }
                if !use_internal {
                    let mut disk = self._disk_image.lock().await;
                    *disk = None;
                    self.leds.set_status(StatusLedMode::Unmounted);
                }
                return false;
            }

            // Build paged menu. total_count is discovered lazily as blocks are fetched.
            let mut menu = Menu::<String<64>, 48, _>::new_paged(menu_title);
            menu.set_scroll_offset(scroll_offset);

            // Load initial blocks into the menu display buffer.
            for (block_idx, chunk) in initial_infos.chunks(16).enumerate() {
                let mut block_strings = Vec::<String<64>, 16>::new();
                for info in chunk {
                    let mut s = String::<64>::new();
                    let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                    let name_trimmed = name_str.trim_matches('\0');
                    let file_path = join_path(current_path.as_str(), name_trimmed);
                    if Some(file_path) == self.mounted_file_path {
                        let _ = s.push('*');
                    }
                    let _ = s.push_str(name_trimmed);
                    if info.is_dir {
                        let _ = s.push_str("/");
                    }
                    
                    let _ = block_strings.push(s);
                }
                menu.set_block(initial_start_block + block_idx, &block_strings);
            }

            // Ensure cursor block is loaded
            loop {
                if let Some(tb) = menu.set_cursor(cursor) {
                    let mut block_infos = Vec::<FileInfo, 16>::new();
                    let fetch_res = if use_internal {
                        self.internal.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                            let _ = block_infos.push(info);
                        }).await
                    } else {
                        self.storage.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                            let _ = block_infos.push(info);
                        }).await
                    };
                    
                    if fetch_res.is_ok() {
                        let mut block_strings = Vec::<String<64>, 16>::new();
                        for info in block_infos.iter() {
                            let mut s = String::<64>::new();
                            let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                            let name_trimmed = name_str.trim_matches('\0');
                            let file_path = join_path(current_path.as_str(), name_trimmed);
                            if Some(file_path) == self.mounted_file_path {
                                let _ = s.push('*');
                            }
                            let _ = s.push_str(name_trimmed);
                            if info.is_dir {
                                let _ = s.push_str("/");
                            }
                            let _ = block_strings.push(s);
                        }
                        menu.set_block(tb, &block_strings);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            let event = loop {
                match menu.run(&mut self.display).await {
                    MenuEvent::LoadBlock(tb) => {
                        let mut block_infos = Vec::<FileInfo, 16>::new();
                        let fetch_res = if use_internal {
                            self.internal.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                let _ = block_infos.push(info);
                            }).await
                        } else {
                            self.storage.list_dir_range(current_path.as_str(), tb * 16, 16, |info| {
                                let _ = block_infos.push(info);
                            }).await
                        };
                        
                        if fetch_res.is_ok() {
                            let mut block_strings = Vec::<String<64>, 16>::new();
                            for info in block_infos.iter() {
                                let mut s = String::<64>::new();
                                let name_str = core::str::from_utf8(&info.name).unwrap_or("???");
                                let name_trimmed = name_str.trim_matches('\0');
                                let file_path = join_path(current_path.as_str(), name_trimmed);
                                if Some(file_path) == self.mounted_file_path {
                                    let _ = s.push('*');
                                }
                                let _ = s.push_str(name_trimmed);
                                if info.is_dir {
                                    let _ = s.push_str("/");
                                }
                                
                                let _ = block_strings.push(s);
                            }
                            menu.set_block(tb, &block_strings);
                        }
                    }
                    MenuEvent::Timeout => {
                        if !use_internal && !self.storage.is_available().await {
                            break MenuEvent::Timeout;
                        }
                    }
                    other => break other,
                }
            };

            cursor = menu.selected_index();
            scroll_offset = menu.scroll_offset();

            match event {
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => {
                    let mut disk = self._disk_image.lock().await;
                    *disk = None;
                    self.leds.set_status(StatusLedMode::Unmounted);
                    if !use_internal && title == "SD Card" {
                        self.sd_path = String::from_str("/").unwrap();
                    }
                    return false;
                }
                MenuEvent::Selected(idx) => {
                    let mut selected_infos = Vec::<FileInfo, 1>::new();
                    let fetch_res = if use_internal {
                        self.internal.list_dir_range(current_path.as_str(), idx, 1, |info| {
                            let _ = selected_infos.push(info);
                        }).await
                    } else {
                        self.storage.list_dir_range(current_path.as_str(), idx, 1, |info| {
                            let _ = selected_infos.push(info);
                        }).await
                    };
                    
                    if fetch_res.is_err() || selected_infos.is_empty() {
                        log::error!("Failed to fetch selected item at index {}", idx);
                        cursor = 0;
                        continue;
                    }
                    let selected = selected_infos[0];
                    if selected.is_dir {
                        self.mounted_file_path = None;
                        let dirname = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let current_path_mut = if use_internal {
                            &mut self.internal_path
                        } else if title == "SD Card" {
                            &mut self.sd_path
                        } else {
                            &mut self.usb_path
                        };
                        let new_path = join_path(current_path_mut.as_str(), dirname);
                        *current_path_mut = new_path;
                        let _ = cursor_stack.push((cursor, menu.scroll_offset()));
                        cursor = 0;
                        scroll_offset = 0;
                    } else {
                        let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let mut full_path = String::<128>::new();
                        let current_path_val = if use_internal {
                            self.internal_path.as_str()
                        } else if title == "SD Card" {
                            self.sd_path.as_str()
                        } else {
                            self.usb_path.as_str()
                        };
                        if current_path_val == "/" {
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        } else {
                            let _ = full_path.push_str(current_path_val);
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        }

                        if Some(full_path.clone()) == self.mounted_file_path {
                            let mut disk = self._disk_image.lock().await;
                            *disk = None;
                            self.leds.set_status(StatusLedMode::Unmounted);
                            self.mounted_file_path = None;
                            continue;
                        }

                        log::info!("Selected file: {:?}", selected);
                        self.display.clear().await;
                        self.display.print(1, "   Mounting...  ").await;
                        self.display.request_redraw();

                        let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                        let mut full_path = String::<128>::new();
                        let current_path_val = if use_internal {
                            self.internal_path.as_str()
                        } else if title == "SD Card" {
                            self.sd_path.as_str()
                        } else {
                            self.usb_path.as_str()
                        };
                        if current_path_val == "/" {
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        } else {
                            let _ = full_path.push_str(current_path_val);
                            let _ = full_path.push_str("/");
                            let _ = full_path.push_str(filename);
                        }

                        let is_d64 = filename.ends_with(".d64") || filename.ends_with(".D64");
                        let is_g64 = filename.ends_with(".g64") || filename.ends_with(".G64");

                        if is_d64 {
                            if use_internal {
                                if let Some(file) = self.internal_menu_file_data(filename) {
                                    let runtime_storage = pico1541_common::storage::RuntimeStorage::Memory(
                                        pico1541_common::storage::MemoryStorage::new(file.data)
                                    );
                                    let d64 = pico1541_common::formats::D64Image::new(runtime_storage);
                                    {
                                        let mut disk = self._disk_image.lock().await;
                                        *disk = Some(pico1541_common::formats::AnyDiskImage::D64(d64));
                                    }
                                    self.leds.set_status(StatusLedMode::NonAccurateDisk);
                                    self.mounted_file_path = Some(full_path.clone());
                                    continue;
                                }
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                let d64 = pico1541_common::formats::D64Image::new(runtime_storage);
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::D64(d64));
                                }
                                self.leds.set_status(StatusLedMode::NonAccurateDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        } else if is_g64 {
                            if use_internal {
                                if let Some(file) = self.internal_menu_file_data(filename) {
                                    let runtime_storage = pico1541_common::storage::RuntimeStorage::Memory(
                                        pico1541_common::storage::MemoryStorage::new(file.data)
                                    );
                                    let g64 = pico1541_common::formats::G64Image::new(runtime_storage);
                                    {
                                        let mut disk = self._disk_image.lock().await;
                                        *disk = Some(pico1541_common::formats::AnyDiskImage::G64(g64));
                                    }
                                    self.leds.set_status(StatusLedMode::AccurateDisk);
                                    self.mounted_file_path = Some(full_path.clone());
                                    continue;
                                }
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                let g64 = pico1541_common::formats::G64Image::new(runtime_storage);
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::G64(g64));
                                }
                                self.leds.set_status(StatusLedMode::AccurateDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        } else {
                            let mut name_len = 0;
                            while name_len < selected.name.len() && selected.name[name_len] != 0 {
                                name_len += 1;
                            }
                            let entry = self.create_directory_entry(&selected.name[..name_len], false, selected.size);
                            let mut dir_source = pico1541_common::storage::PagedDirectorySource::new(filename.as_bytes());
                            let mut block_entries = Vec::<pico1541_common::storage::DirectoryEntry, 16>::new();
                            let _ = block_entries.push(entry);
                            dir_source.set_block(0, &block_entries);

                            let mut vdisk = pico1541_common::formats::VirtualDiskImage::new(dir_source);

                            if use_internal {
                                if let Some(file) = self.internal_menu_file_data(filename) {
                                    let runtime_storage = pico1541_common::storage::RuntimeStorage::Memory(
                                        pico1541_common::storage::MemoryStorage::new(file.data)
                                    );
                                    vdisk.file_storage = Some(runtime_storage);
                                    vdisk.set_shortcut_entry(Some(entry));
                                    {
                                        let mut disk = self._disk_image.lock().await;
                                        *disk = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                                    }
                                    self.leds.set_status(StatusLedMode::VirtualDisk);
                                    self.mounted_file_path = Some(full_path.clone());
                                    continue;
                                }
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                vdisk.file_storage = Some(runtime_storage);
                                vdisk.set_shortcut_entry(Some(entry));
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                                }
                                self.leds.set_status(StatusLedMode::VirtualDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        }
                    }
                }
                MenuEvent::Back => {
                    let current_path_mut = if use_internal {
                        &mut self.internal_path
                    } else if title == "SD Card" {
                        &mut self.sd_path
                    } else {
                        &mut self.usb_path
                    };
                    if current_path_mut.as_str() == "/" {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        self.leds.set_status(StatusLedMode::Unmounted);
                        self.mounted_file_path = None;
                        return false;
                    } else {
                        let parent = parent_dir(current_path_mut.as_str());
                        *current_path_mut = String::from_str(parent).unwrap();
                        self.mounted_file_path = None;
                        let state = cursor_stack.pop().unwrap_or((0, 0));
                        cursor = state.0;
                        scroll_offset = state.1;
                        continue;
                    }
                }
                MenuEvent::Home => {
                    let mut disk = self._disk_image.lock().await;
                    *disk = None;
                    self.leds.set_status(StatusLedMode::Unmounted);
                    self.mounted_file_path = None;
                    return true;
                }
            }
        }
    }

    async fn settings_menu(&mut self) -> bool {
        let mut items = Vec::<&'static str, 4>::new();
        let _ = items.push("Drive ID");
        let _ = items.push("Monitor");
        let _ = items.push("Hardware Test");
        
        let mut menu = Menu::new("--- Settings ---", items);

        loop {
            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    match idx {
                        0 => {
                            if self.device_id_menu().await {
                                return true;
                            }
                        }
                        1 => {
                            if self.monitor_page().await {
                                return true;
                            }
                        }
                        2 => {
                            let mut test = HardwareTestTask::new(
                                self.display.clone(),
                                self.leds.clone(),
                                self.indicator.clone(),
                            );
                            test.run().await;
                        }
                        _ => {
                            log::info!("Selected setting: {:?}", menu.selected_item());
                        }
                    }
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => {}
            }
        }
    }

    async fn monitor_page(&mut self) -> bool {
        loop {
            self.display.clear().await;
            self.display.print(0, "-- System Monitor --").await;

            let speed_hz = self.config.emu_speed_hz.load(Ordering::Relaxed);
            let c0_avg = self.config.core0_avg_raw.load(Ordering::Relaxed);
            let c0_max = self.config.core0_max_raw.load(Ordering::Relaxed);
            let c1_avg = self.config.core1_avg_raw.load(Ordering::Relaxed);
            let c1_max = self.config.core1_max_raw.load(Ordering::Relaxed);

            let mut s_speed = String::<32>::new();
            if speed_hz >= 1_000_000 {
                let mhz_int = speed_hz / 1_000_000;
                let mhz_frac = (speed_hz % 1_000_000) / 10_000;
                let _ = core::fmt::write(&mut s_speed, format_args!("CPU Speed: {}.{:02} MHz", mhz_int, mhz_frac));
            } else {
                let _ = core::fmt::write(&mut s_speed, format_args!("CPU Speed: {} Hz", speed_hz));
            }
            self.display.print(2, &s_speed).await;

            let c0_load = if c0_max > 0 {
                100u32.saturating_sub((c0_avg * 100) / c0_max)
            } else {
                0
            };
            let mut s_c0 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c0, format_args!("Core 0 Load: {}%", c0_load));
            self.display.print(3, &s_c0).await;

            let c1_load = if c1_max > 0 {
                100u32.saturating_sub((c1_avg * 100) / c1_max)
            } else {
                0
            };
            let mut s_c1 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c1, format_args!("Core 1 Load: {}%", c1_load));
            self.display.print(4, &s_c1).await;

            self.display.print(6, " [Back] ").await;
            self.display.request_redraw();

            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(500),
                crate::ACTION_CHANNEL.receive()
            ).await {
                Ok(UiAction::Back) => return false,
                Ok(UiAction::Home) => return true,
                _ => {}
            }
        }
    }

    async fn device_id_menu(&mut self) -> bool {
        let mut cursor = (self.config.device_id.load(portable_atomic::Ordering::Relaxed) - 8) as usize;
        loop {
            let current_id = self.config.device_id.load(portable_atomic::Ordering::Relaxed);
            let mut items = Vec::<String<16>, 4>::new();
            
            for id in 8..=11 {
                let mut s = String::<16>::new();
                if id == current_id {
                    let _ = s.push_str("* ");
                } else {
                    let _ = s.push_str("  ");
                }
                let _ = core::fmt::write(&mut s, format_args!("{}", id));
                let _ = items.push(s);
            }

            let mut menu = Menu::new("--- Drive ID ---", items);
            menu.set_cursor(cursor);

            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    cursor = idx;
                    let new_id = (idx + 8) as u8;
                    self.config.update_device_id(new_id);
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => {
                    cursor = menu.selected_index();
                }
            }
        }
    }
}
