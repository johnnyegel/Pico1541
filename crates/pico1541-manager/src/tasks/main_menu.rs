use crate::driver::{DisplayDriver, RgbLedDriver, StatusLedMode, IndicatorDriver, FileStorageDriver, FileInfo};
use crate::tasks::menu::{Menu, MenuEvent};
// Removed HardwareTestTask
use crate::UiAction;
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

pub struct MainMenuTask<D, L, Ind, S, Internal: crate::driver::FileStorageDriver, File>
where
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    display: D,
    leds: L,
    _indicator: Ind,
    storage: S,
    internal: Internal,
    _disk_image: &'static Mutex<CriticalSectionRawMutex, Option<pico1541_common::AnyDiskImage<pico1541_common::storage::RuntimeStorage<'static, File, Internal::File>, pico1541_common::storage::PagedDirectorySource<48>>>>,
    viewed_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    config: &'static SnooperConfig,
    sd_path: String<128>,
    internal_path: String<128>,
    usb_path: String<128>,
    mounted_file_path: Option<String<128>>,
    rom_name: String<32>,
}

impl<D, L, Ind, S, Internal, File> MainMenuTask<D, L, Ind, S, Internal, File>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    pub fn new(
        display: D,
        leds: L,
        indicator: Ind,
        storage: S,
        internal: Internal,
        disk_image: &'static Mutex<CriticalSectionRawMutex, Option<pico1541_common::AnyDiskImage<pico1541_common::storage::RuntimeStorage<'static, File, Internal::File>, pico1541_common::storage::PagedDirectorySource<48>>>>,
        viewed_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
        config: &'static SnooperConfig,
        rom_name: String<32>,
    ) -> Self {
        Self {
            display,
            leds,
            _indicator: indicator,
            storage,
            internal,
            _disk_image: disk_image,
            viewed_dir,
            mounted_dir,
            config,
            sd_path: String::from_str("/").unwrap(),
            internal_path: String::from_str("/files").unwrap(),
            usb_path: String::from_str("/").unwrap(),
            mounted_file_path: None,
            rom_name,
        }
    }



    pub async fn run(&mut self) -> ! {
        // Initialize the status LED to the default startup mode (Unmounted - Red)
        self.leds.set_status(StatusLedMode::Unmounted);
        {
            let mut disk = self._disk_image.lock().await;
            *disk = None;
            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
        }

        // 1. Boot Screen
        self.display.clear().await;
        self.display.print_large(1, "  Pico1541").await; 
        
        let mut id_str = heapless::String::<16>::new();
        use core::fmt::Write;
        write!(id_str, "    Drive ID: {}", self.config.device_id.load(Ordering::Relaxed)).unwrap();
        self.display.print(4, id_str.as_str()).await;
        
        let mut rom_str = heapless::String::<64>::new();
        write!(rom_str, "{}", self.rom_name.as_str()).unwrap();
        self.display.print(5, rom_str.as_str()).await;
        
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
                MenuEvent::Monitor => {
                    let _ = self.monitor_page().await;
                    continue;
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
                MenuEvent::ContextAction(_) => {}
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
                self.internal_path.clone()
            } else if title == "SD Card" {
                self.sd_path.clone()
            } else {
                self.usb_path.clone()
            };

            let path_str = current_path.as_str();

            let current_block = cursor / 16;
            let initial_start_block = current_block.saturating_sub(1);
            let initial_start_idx = initial_start_block * 16;

            // Load the entries around the cursor position (up to 64 entries = 4 blocks)
            let mut initial_infos = Vec::<FileInfo, 64>::new();
            let load_res = if use_internal {
                self.internal.list_dir_range(current_path.as_str(), initial_start_idx, 64, |info| {
                    let _ = initial_infos.push(info);
                }).await
            } else {
                self.storage.list_dir_range(current_path.as_str(), initial_start_idx, 64, |info| {
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
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    self.leds.set_status(StatusLedMode::Unmounted);
                }
                return false;
            }

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
                    let entry = pico1541_common::storage::DirectoryEntry::from_file_info(&info.name[..name_len], info.is_dir, info.size);
                    let _ = block_entries.push(entry);
                }
                dir_source.set_block(initial_start_block + block_idx, &block_entries);
            }

            let vdisk = pico1541_common::formats::VirtualDiskImage::new(dir_source, dir_name.as_bytes());

            if self.mounted_file_path.is_none() {
                {
                    let mut mounted_dir_guard = self.mounted_dir.lock().await;
                    mounted_dir_guard.0 = current_path.clone();
                    mounted_dir_guard.1 = use_internal;
                }
                {
                    let mut disk_lock = self._disk_image.lock().await;
                    *disk_lock = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                }
                self.leds.set_status(StatusLedMode::VirtualDisk);
            }

            {
                let mut viewed_dir_guard = self.viewed_dir.lock().await;
                viewed_dir_guard.0 = current_path.clone();
                viewed_dir_guard.1 = use_internal;
            }

            let mut menu_title = String::<64>::new();
            if path_str == "/" && !use_internal {
                if let Some(label) = self.storage.volume_label() {
                    let _ = core::fmt::write(&mut menu_title, format_args!("SD: {}", label.as_str()));
                } else {
                    let _ = menu_title.push_str("SD CARD");
                }
            } else {
                let _ = menu_title.push_str(dir_name);
            }
            let mut menu = Menu::<String<64>, 64, _>::new_paged(menu_title.clone());
            menu.set_scroll_offset(scroll_offset);

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
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                            self.leds.set_status(StatusLedMode::Unmounted);
                            return false;
                        } else {
                            let parent = parent_dir(current_path_mut.as_str());
                            *current_path_mut = String::from_str(parent).unwrap();
                            let state = cursor_stack.pop().unwrap_or((0, 0));
                            cursor = state.0;
                            scroll_offset = state.1;
                            self.mounted_file_path = None;
                            continue;
                        }
                    }
                    MenuEvent::Home => {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                        self.leds.set_status(StatusLedMode::Unmounted);
                        return true;
                    }
                    MenuEvent::Monitor => {
                        if self.monitor_page().await {
                            let mut disk = self._disk_image.lock().await;
                            *disk = None;
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                            self.leds.set_status(StatusLedMode::Unmounted);
                            return true;
                        }
                        self.display.request_redraw();
                        continue;
                    }
                    _ => {}
                }
                if !use_internal {
                    let mut disk = self._disk_image.lock().await;
                    *disk = None;
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
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
                        
                        if let Some(tb) = menu.prefetch_target() {
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
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
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
                            let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
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
                                let file = self.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File2(file);
                                let d64 = pico1541_common::formats::D64Image::new(runtime_storage);
                                    {
                                        let mut disk = self._disk_image.lock().await;
                                        *disk = Some(pico1541_common::formats::AnyDiskImage::D64(d64));
                                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                                    }
                                    self.leds.set_status(StatusLedMode::NonAccurateDisk);
                                    self.mounted_file_path = Some(full_path.clone());
                                    continue;
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                let d64 = pico1541_common::formats::D64Image::new(runtime_storage);
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::D64(d64));
                                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                                }
                                self.leds.set_status(StatusLedMode::NonAccurateDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        } else if is_g64 {
                            if use_internal {
                                let file = self.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File2(file);
                                let g64 = pico1541_common::formats::G64Image::new(runtime_storage);
                                    {
                                        let mut disk = self._disk_image.lock().await;
                                        *disk = Some(pico1541_common::formats::AnyDiskImage::G64(g64));
                                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                                    }
                                    self.leds.set_status(StatusLedMode::AccurateDisk);
                                    self.mounted_file_path = Some(full_path.clone());
                                    continue;
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                let g64 = pico1541_common::formats::G64Image::new(runtime_storage);
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::G64(g64));
                                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
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
                            let entry = pico1541_common::storage::DirectoryEntry::from_file_info(&selected.name[..name_len], false, selected.size);
                            let mut dir_source = pico1541_common::storage::PagedDirectorySource::new(filename.as_bytes());
                            let mut block_entries = Vec::<pico1541_common::storage::DirectoryEntry, 16>::new();
                            let _ = block_entries.push(entry);
                            dir_source.set_block(0, &block_entries);

                            let mut vdisk = pico1541_common::formats::VirtualDiskImage::new(dir_source, "INTERNAL".as_bytes());

                            if use_internal {
                                let file = self.internal.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File2(file);
                                vdisk.file_storage = Some(runtime_storage);
                                vdisk.set_shortcut_entry(Some(entry));
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                                }
                                self.leds.set_status(StatusLedMode::VirtualDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            } else {
                                let file_handle = self.storage.open_file_handle(full_path.as_str());
                                let runtime_storage = pico1541_common::storage::RuntimeStorage::File(file_handle);
                                vdisk.file_storage = Some(runtime_storage);
                                vdisk.set_shortcut_entry(Some(entry));
                                {
                                    let mut disk = self._disk_image.lock().await;
                                    *disk = Some(pico1541_common::formats::AnyDiskImage::VirtualDisk(vdisk));
                                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageMounted);
                                }
                                self.leds.set_status(StatusLedMode::VirtualDisk);
                                self.mounted_file_path = Some(full_path.clone());
                                continue;
                            }
                        }
                    }
                }
                MenuEvent::ContextAction(idx) => {
                    if use_internal { continue; }
                    let mut selected_infos = Vec::<FileInfo, 1>::new();
                    let fetch_res = self.storage.list_dir_range(current_path.as_str(), idx, 1, |info| {
                        let _ = selected_infos.push(info);
                    }).await;
                    
                    if fetch_res.is_err() || selected_infos.is_empty() {
                        continue;
                    }
                    let selected = selected_infos[0];
                    if selected.is_dir { continue; }

                    let filename = core::str::from_utf8(&selected.name).unwrap_or("???").trim_matches('\0');
                    if filename.ends_with(".bin") || filename.ends_with(".rom") || filename.ends_with(".BIN") || filename.ends_with(".ROM") {
                        if use_internal {
                            let mut options = Vec::<String<32>, 2>::new();
                            options.push(String::from_str("Abort").unwrap()).unwrap();
                            options.push(String::from_str("Delete").unwrap()).unwrap();
                            let mut ctx_menu = Menu::new("Delete ROM?", options);
                            loop {
                                match ctx_menu.run(&mut self.display).await {
                                    MenuEvent::Selected(opt_idx) => {
                                        if opt_idx == 1 {
                                            let _ = crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::DeleteRom(heapless::String::<32>::from_str(filename).unwrap()));
                                            self.display.clear().await;
                                            self.display.print(2, "   ROM Deleted!").await;
                                            self.display.request_redraw();
                                            Timer::after(Duration::from_millis(1500)).await;
                                        }
                                        break;
                                    }
                                    MenuEvent::Back | MenuEvent::Home => break,
                                    MenuEvent::Monitor => {
                                        let _ = self.monitor_page().await;
                                        self.display.request_redraw();
                                        continue;
                                    }
                                    MenuEvent::Timeout => continue,
                                    MenuEvent::ContextAction(_) => continue,
                                    MenuEvent::LoadBlock(_) => {}
                                }
                            }
                        } else if selected.size >= 16384 && selected.size % 16384 == 0 && selected.size <= 65536 {
                            let mut options = Vec::<String<32>, 5>::new();
                            options.push(String::from_str("Abort").unwrap()).unwrap();
                            let num_banks = selected.size / 16384;
                            if num_banks == 1 {
                                options.push(String::from_str("Install").unwrap()).unwrap();
                            } else {
                                for b in 1..=num_banks {
                                    let mut s = String::<32>::new();
                                    core::fmt::write(&mut s, format_args!("Install Bank {}", b)).unwrap();
                                    options.push(s).unwrap();
                                }
                            }
                            let mut ctx_menu = Menu::new("Install ROM?", options);
                            loop {
                                match ctx_menu.run(&mut self.display).await {
                                    MenuEvent::Selected(opt_idx) => {
                                        if opt_idx > 0 {
                                            let bank = opt_idx as u32; // 1-indexed bank
                                            let path_str = heapless::String::<128>::from_str(current_path.as_str()).unwrap();
                                            self.install_rom_from_sd(path_str.as_str(), filename, bank).await;
                                        }
                                        break;
                                    }
                                    MenuEvent::Back | MenuEvent::Home => break,
                                    MenuEvent::Monitor => {
                                        let _ = self.monitor_page().await;
                                        self.display.request_redraw();
                                        continue;
                                    }
                                    MenuEvent::Timeout => continue,
                                    MenuEvent::ContextAction(_) => continue, // ignore repeated hold events
                                    MenuEvent::LoadBlock(_) => {} // context menu doesn't need blocks
                                }
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
                    let is_root = if use_internal {
                        current_path_mut.as_str() == "/files"
                    } else {
                        current_path_mut.as_str() == "/"
                    };

                    if is_root {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
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
                    let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    self.leds.set_status(StatusLedMode::Unmounted);
                    self.mounted_file_path = None;
                    return true;
                }
                MenuEvent::Monitor => {
                    if self.monitor_page().await {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                        self.leds.set_status(StatusLedMode::Unmounted);
                        self.mounted_file_path = None;
                        return true;
                    }
                    self.display.request_redraw();
                    continue;
                }
            }
        }
    }

    async fn settings_menu(&mut self) -> bool {
        let mut cursor = 0;
        loop {
            let mut items = Vec::<String<32>, 5>::new();
            
            let mut id_str = String::<32>::new();
            let _ = core::fmt::write(&mut id_str, format_args!("Drive ID: {}", self.config.device_id.load(portable_atomic::Ordering::Relaxed)));
            let _ = items.push(id_str);

            let _ = items.push(String::from_str("Select ROM").unwrap());

            let _ = items.push(String::from_str("Monitor").unwrap());
            let _ = items.push(String::from_str("Store Settings").unwrap());

            let mut menu = Menu::new("--- Settings ---", items);
            menu.set_cursor(cursor);

            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    cursor = idx;
                    match idx {
                        0 => {
                            if self.device_id_menu().await { return true; }
                        }
                        1 => {
                            if self.select_boot_rom_menu().await { return true; }
                        }
                        2 => {
                            if self.monitor_page().await { return true; }
                        }
                        3 => {
                            let current_id = self.config.device_id.load(portable_atomic::Ordering::Relaxed);
                            let _ = crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::SaveDriveId(current_id));
                            
                            self.display.clear().await;
                            self.display.print(2, "   Settings Saved!").await;
                            self.display.request_redraw();
                            Timer::after(Duration::from_millis(1500)).await;
                        }
                        _ => {}
                    }
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
                MenuEvent::Monitor => {
                    if self.monitor_page().await { return true; }
                    self.display.request_redraw();
                    continue;
                }
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => { cursor = menu.selected_index(); }
                _ => {}
            }
        }
    }

    async fn monitor_page(&mut self) -> bool {
        let free_flash = self.internal.free_space().await;
        loop {
            self.display.clear().await;
            self.display.print(0, "-- System Monitor --").await;

            let speed_hz = self.config.emu_speed_hz.load(Ordering::Relaxed);
            let c0_avg = self.config.core0_avg_raw.load(Ordering::Relaxed);
            let _c0_max = self.config.core0_max_raw.load(Ordering::Relaxed);
            let c1_avg = self.config.core1_avg_raw.load(Ordering::Relaxed);
            let c1_max = self.config.core1_max_raw.load(Ordering::Relaxed);

            let mut s_speed = String::<32>::new();
            if speed_hz >= 1_000_000 {
                let mhz_int = speed_hz / 1_000_000;
                let mhz_frac = (speed_hz % 1_000_000) / 10_000;
                let _ = core::fmt::write(&mut s_speed, format_args!("Emu clk : {}.{:02} MHz", mhz_int, mhz_frac));
            } else {
                let _ = core::fmt::write(&mut s_speed, format_args!("Emu clk : {} Hz", speed_hz));
            }
            self.display.print(1, &s_speed).await;

            // core0_avg_raw now stores average SYST cycles per 1541-tick.
            // 250 SYST cycles = 100% load (since 250MHz sys clock / 1MHz 1541 clock)
            let c0_load = (c0_avg * 100) / 250;
            let mut s_c0 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c0, format_args!("Emu load: {}%", c0_load));
            self.display.print(2, &s_c0).await;

            let c1_load = if c1_max > 0 {
                100u32.saturating_sub(((c1_avg as u64 * 100) / c1_max as u64) as u32)
            } else {
                0
            };
            let mut s_c1 = String::<32>::new();
            let _ = core::fmt::write(&mut s_c1, format_args!("Mgr load: {}%", c1_load));
            self.display.print(3, &s_c1).await;

            if let Some(free_bytes) = free_flash {
                let mut s_flash = String::<32>::new();
                let _ = core::fmt::write(&mut s_flash, format_args!("Flash free: {} KB", free_bytes / 1024));
                self.display.print(4, &s_flash).await;
            }

            self.display.print(6, " [Back] ").await;
            self.display.request_redraw();

            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(200),
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
                    let _ = s.push_str("*");
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
                MenuEvent::Monitor => {
                    if self.monitor_page().await { return true; }
                    self.display.request_redraw();
                    continue;
                }
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => {
                    cursor = menu.selected_index();
                }
                MenuEvent::ContextAction(_) => {}
            }
        }
    }

    async fn install_rom_from_sd(&mut self, current_path: &str, filename: &str, bank: u32) {
        use embedded_io_async::SeekFrom;
        self.display.clear().await;
        self.display.print(1, " Installing ROM...").await;
        self.display.request_redraw();

        let full_path = join_path(current_path, filename);
        let mut file = self.storage.open_file_handle(full_path.as_str());

        let offset = (bank - 1) * 16384;
        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)).await {
            log::error!("Failed to seek to ROM bank: {:?}", e);
            self.display.print(2, " Seek Error!").await;
            self.display.request_redraw();
            Timer::after(Duration::from_secs(2)).await;
            return;
        }

        let mut dest_filename = String::<32>::new();
        if filename.ends_with(".bin") || filename.ends_with(".BIN") {
            let base = &filename[..filename.len() - 4];
            let _ = core::fmt::write(&mut dest_filename, format_args!("{}-B{}.bin", base, bank));
        } else if filename.ends_with(".rom") || filename.ends_with(".ROM") {
            let base = &filename[..filename.len() - 4];
            let _ = core::fmt::write(&mut dest_filename, format_args!("{}-B{}.rom", base, bank));
        } else {
            let _ = dest_filename.push_str(filename);
        }

        let mut buf = [0u8; 256];
        let mut bytes_read = 0;

        let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomStart {
            filename: dest_filename.clone(),
            size: 16384,
            offset: 0,
        }).await;

        while bytes_read < 16384 {
            let to_read = (16384 - bytes_read).min(256);
            if let Ok(n) = file.read(&mut buf[..to_read]).await {
                if n == 0 { break; }
                let mut chunk = heapless::Vec::new();
                let _ = chunk.extend_from_slice(&buf[..n]);
                let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomChunk(chunk)).await;
                bytes_read += n;
                
                if bytes_read % 2048 == 0 {
                    let mut prog = String::<32>::new();
                    let _ = core::fmt::write(&mut prog, format_args!(" {} / 16384", bytes_read));
                    self.display.print(3, &prog).await;
                    self.display.request_redraw();
                }
            } else {
                break;
            }
        }

        let _ = crate::SETTINGS_CHANNEL.send(crate::SettingsCommand::InstallRomFinish).await;
        
        self.display.print(4, " Done!").await;
        self.display.request_redraw();
        Timer::after(Duration::from_secs(1)).await;
    }

    async fn select_boot_rom_menu(&mut self) -> bool {
        let mut cursor = 0;
        loop {
            let mut items = Vec::<String<32>, 48>::new();
            let mut infos = Vec::<FileInfo, 48>::new();
            
            let _ = self.internal.list_dir_range("/roms", 0, 48, |info| {
                let _ = infos.push(info);
            }).await;

            let mut roms_added = 0;
            for info in &infos {
                if !info.is_dir {
                    let name = core::str::from_utf8(&info.name).unwrap_or("???").trim_matches('\0');
                    if name.ends_with(".bin") || name.ends_with(".rom") || name.ends_with(".BIN") || name.ends_with(".ROM") {
                        let mut s = String::<32>::new();
                        if name == self.rom_name.as_str() {
                            let _ = s.push('*');
                        }
                        let _ = s.push_str(name);
                        let _ = items.push(s);
                        roms_added += 1;
                    }
                }
            }

            if roms_added == 0 {
                let _ = items.push(String::from_str("No roms installed").unwrap());
            }

            let mut menu = Menu::new("--- Boot ROM ---", items);
            menu.set_cursor(cursor);

            match menu.run(&mut self.display).await {
                MenuEvent::Selected(idx) => {
                    if roms_added == 0 {
                        return false;
                    }
                    let name_with_star = menu.get_item(idx).unwrap();
                    let name = if name_with_star.starts_with('*') {
                        &name_with_star[1..]
                    } else {
                        name_with_star.as_str()
                    };
                    self.rom_name = String::from_str(name).unwrap();
                    let _ = crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::SaveBootRom(self.rom_name.clone()));
                    let _ = crate::SETTINGS_CHANNEL.try_send(crate::SettingsCommand::LoadRomToMemory(self.rom_name.clone()));
                    cursor = idx;
                }
                MenuEvent::Back => return false,
                MenuEvent::Home => return true,
                MenuEvent::Monitor => {
                    if self.monitor_page().await { return true; }
                    self.display.request_redraw();
                    continue;
                }
                MenuEvent::LoadBlock(_) => {}
                MenuEvent::Timeout => { cursor = menu.selected_index(); }
                _ => {}
            }
        }
    }


}
