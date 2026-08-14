use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver, FileStorageDriver};
// Removed HardwareTestTask
use heapless::String;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use core::str::FromStr;
use pico1541_common::snooper::SnooperConfig;

pub mod monitor;
pub mod boot;
pub mod home;
pub mod settings;
pub mod browser;

pub struct UiContext<'a, D, L, Ind, S, Internal, File>
where
    Internal: crate::driver::FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    pub display: &'a mut D,
    pub leds: &'a mut L,
    pub _indicator: &'a mut Ind,
    pub storage: &'a mut S,
    pub internal: &'a mut Internal,
    pub _disk_image: &'a Mutex<CriticalSectionRawMutex, Option<crate::AnyDiskImage<crate::storage::RuntimeStorage<'static, File, Internal::File>, crate::storage::PagedDirectorySource<48>>>>,
    pub viewed_dir: &'a Mutex<CriticalSectionRawMutex, (String<128>, bool)>,
    pub mounted_dir: &'a Mutex<CriticalSectionRawMutex, (String<128>, bool)>,
    pub config: &'a SnooperConfig,
    pub sd_path: &'a mut String<128>,
    pub internal_path: &'a mut String<128>,
    pub usb_path: &'a mut String<128>,
    pub mounted_file_path: &'a mut Option<String<128>>,
    pub rom_name: &'a mut String<32>,
}


pub enum PageState {
    BootScreen,
    MainMenu,
    SdCardMenu,
    InternalMenu,
    Settings,
    Monitor,
    DeviceIdSelect,
    BootRomSelect,
}

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

#[allow(clippy::type_complexity)]
pub struct MainMenuTask<D, L, Ind, S, Internal: crate::driver::FileStorageDriver, File>
where
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    display: D,
    leds: L,
    _indicator: Ind,
    storage: S,
    internal: Internal,
    _disk_image: &'static Mutex<CriticalSectionRawMutex, Option<crate::AnyDiskImage<crate::storage::RuntimeStorage<'static, File, Internal::File>, crate::storage::PagedDirectorySource<48>>>>,
    viewed_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    mounted_dir: &'static Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)>,
    config: &'static SnooperConfig,
    sd_path: String<128>,
    internal_path: String<128>,
    usb_path: String<128>,
    mounted_file_path: Option<String<128>>,
    rom_name: String<32>,
}

#[allow(clippy::type_complexity)]
impl<D, L, Ind, S, Internal, File> MainMenuTask<D, L, Ind, S, Internal, File>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver<File = File> + Clone,
    Internal: FileStorageDriver,
    File: embedded_io_async::Read + embedded_io_async::Seek + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display: D,
        leds: L,
        indicator: Ind,
        storage: S,
        internal: Internal,
        disk_image: &'static Mutex<CriticalSectionRawMutex, Option<crate::AnyDiskImage<crate::storage::RuntimeStorage<'static, File, Internal::File>, crate::storage::PagedDirectorySource<48>>>>,
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
        let mut state = PageState::BootScreen;

        loop {
            state = match state {
                PageState::BootScreen => {
                    {
                        let mut disk = self._disk_image.lock().await;
                        *disk = None;
                        let _ = crate::CMD_CHANNEL.try_send(crate::InternalCommand::ImageUnmounted);
                    }
                    boot::run_boot_screen(&mut self.display, &mut self.leds, self.config, &self.rom_name).await
                }
                PageState::MainMenu => {
                    home::run_main_menu(&mut self.display, &mut self.storage, &mut self.internal, &mut self.sd_path).await
                }
                PageState::Monitor => {
                    let _ = monitor::run_monitor_page(&mut self.display, &mut self.internal, self.config).await;
                    PageState::MainMenu
                }
                PageState::SdCardMenu => {
                    let mut ctx = UiContext {
                        display: &mut self.display,
                        leds: &mut self.leds,
                        _indicator: &mut self._indicator,
                        storage: &mut self.storage,
                        internal: &mut self.internal,
                        _disk_image: self._disk_image,
                        viewed_dir: self.viewed_dir,
                        mounted_dir: self.mounted_dir,
                        config: self.config,
                        sd_path: &mut self.sd_path,
                        internal_path: &mut self.internal_path,
                        usb_path: &mut self.usb_path,
                        mounted_file_path: &mut self.mounted_file_path,
                        rom_name: &mut self.rom_name,
                    };
                    browser::run_sd_card_menu(&mut ctx).await
                }
                PageState::InternalMenu => {
                    let mut ctx = UiContext {
                        display: &mut self.display,
                        leds: &mut self.leds,
                        _indicator: &mut self._indicator,
                        storage: &mut self.storage,
                        internal: &mut self.internal,
                        _disk_image: self._disk_image,
                        viewed_dir: self.viewed_dir,
                        mounted_dir: self.mounted_dir,
                        config: self.config,
                        sd_path: &mut self.sd_path,
                        internal_path: &mut self.internal_path,
                        usb_path: &mut self.usb_path,
                        mounted_file_path: &mut self.mounted_file_path,
                        rom_name: &mut self.rom_name,
                    };
                    browser::run_internal_menu(&mut ctx).await
                }
                PageState::Settings => {
                    settings::run_settings_menu(&mut self.display, &mut self.internal, self.config).await
                }
                PageState::DeviceIdSelect => {
                    settings::run_device_id_menu(&mut self.display, &mut self.internal, self.config).await
                }
                PageState::BootRomSelect => {
                    settings::run_select_boot_rom_menu(&mut self.display, &mut self.internal, self.config, &mut self.rom_name).await
                }
            }
        }
    }

}
