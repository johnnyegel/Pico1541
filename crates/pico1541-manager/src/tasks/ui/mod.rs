use crate::driver::{DisplayDriver, FileStorageDriver, IndicatorDriver, RgbLedDriver};
// Removed HardwareTestTask
use crate::{InternalCommand, SettingsCommand, UiAction};
use core::str::FromStr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use heapless::String;
use pico1541_common::snooper::SnooperConfig;
use pico1541_common::DriveLocation;

use embassy_sync::channel::{Receiver, Sender};

pub mod boot;
pub mod browser;
pub mod home;
pub mod monitor;
pub mod settings;
pub mod virtual_directory;

#[derive(Clone, Copy)]
pub struct UiContext {
    pub action_rx: Receiver<'static, CriticalSectionRawMutex, UiAction, 16>,
    pub cmd_tx: Sender<'static, CriticalSectionRawMutex, InternalCommand, 32>,
    pub settings_tx: Sender<'static, CriticalSectionRawMutex, SettingsCommand, 2>,
}

pub struct MenuContext<'a, D, L, Ind, S, Internal>
where
    Internal: FileStorageDriver,
{
    pub display: &'a mut D,
    pub leds: &'a mut L,
    pub _indicator: &'a mut Ind,
    pub storage: &'a mut S,
    pub internal: &'a mut Internal,
    pub location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
    pub config: &'a SnooperConfig,
    pub sd_path: &'a mut String<128>,
    pub internal_path: &'a mut String<128>,
    pub usb_path: &'a mut String<128>,
    pub mounted_file_path: &'a mut Option<String<128>>,
    pub rom_name: &'a mut String<32>,
    pub sys_clock_freq: u32,
    pub context: UiContext,
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
pub struct MainMenuTask<D, L, Ind, S, Internal: FileStorageDriver> {
    display: D,
    leds: L,
    _indicator: Ind,
    storage: S,
    internal: Internal,
    location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
    config: &'static SnooperConfig,
    sd_path: String<128>,
    internal_path: String<128>,
    usb_path: String<128>,
    mounted_file_path: Option<String<128>>,
    rom_name: String<32>,
    sys_clock_freq: u32,
    context: UiContext,
}

#[allow(clippy::type_complexity)]
impl<D, L, Ind, S, Internal> MainMenuTask<D, L, Ind, S, Internal>
where
    D: DisplayDriver + Clone,
    L: RgbLedDriver + Clone,
    Ind: IndicatorDriver + Clone,
    S: FileStorageDriver + Clone,
    Internal: FileStorageDriver,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display: D,
        leds: L,
        indicator: Ind,
        storage: S,
        internal: Internal,
        location: &'static Mutex<CriticalSectionRawMutex, DriveLocation>,
        config: &'static SnooperConfig,
        rom_name: String<32>,
        sys_clock_freq: u32,
        context: UiContext,
    ) -> Self {
        Self {
            display,
            leds,
            _indicator: indicator,
            storage,
            internal,
            location,
            config,
            sd_path: String::from_str("/").unwrap(),
            internal_path: String::from_str("/files").unwrap(),
            usb_path: String::from_str("/").unwrap(),
            mounted_file_path: None,
            rom_name,
            sys_clock_freq,
            context,
        }
    }

    pub async fn run(&mut self) -> ! {
        let mut state = PageState::BootScreen;

        loop {
            match state {
                PageState::MainMenu => {
                    let loc = self.location.lock().await.clone();
                    match loc {
                        DriveLocation::VirtualDirectory { use_internal, .. }
                        | DriveLocation::DiskImage { use_internal, .. } => {
                            state = if use_internal {
                                PageState::InternalMenu
                            } else {
                                PageState::SdCardMenu
                            };
                            continue;
                        }
                        DriveLocation::Root => {}
                    }
                }
                PageState::SdCardMenu | PageState::InternalMenu => {
                    let loc = self.location.lock().await.clone();
                    if loc.is_root() {
                        state = PageState::MainMenu;
                        continue;
                    }
                }
                _ => {}
            }

            state = match state {
                PageState::BootScreen => {
                    let _ = self.context.cmd_tx.try_send(crate::InternalCommand::Unmount);
                    boot::run_boot_screen(
                        &mut self.display,
                        &mut self.leds,
                        self.config,
                        &self.rom_name,
                    )
                    .await
                }
                PageState::MainMenu => {
                    home::run_main_menu(
                        &mut self.display,
                        &mut self.storage,
                        &mut self.internal,
                        &mut self.sd_path,
                        self.location,
                        &self.context,
                    )
                    .await
                }
                PageState::Monitor => {
                    let _ = monitor::run_monitor_page(
                        &mut self.display,
                        &mut self.internal,
                        self.config,
                        self.sys_clock_freq,
                        &self.context,
                    )
                    .await;
                    PageState::MainMenu
                }
                PageState::SdCardMenu => {
                    let mut ctx = MenuContext {
                        display: &mut self.display,
                        leds: &mut self.leds,
                        _indicator: &mut self._indicator,
                        storage: &mut self.storage,
                        internal: &mut self.internal,
                        location: self.location,
                        config: self.config,
                        sd_path: &mut self.sd_path,
                        internal_path: &mut self.internal_path,
                        usb_path: &mut self.usb_path,
                        mounted_file_path: &mut self.mounted_file_path,
                        rom_name: &mut self.rom_name,
                        sys_clock_freq: self.sys_clock_freq,
                        context: self.context,
                    };
                    browser::run_sd_card_menu(&mut ctx).await
                }
                PageState::InternalMenu => {
                    let mut ctx = MenuContext {
                        display: &mut self.display,
                        leds: &mut self.leds,
                        _indicator: &mut self._indicator,
                        storage: &mut self.storage,
                        internal: &mut self.internal,
                        location: self.location,
                        config: self.config,
                        sd_path: &mut self.sd_path,
                        internal_path: &mut self.internal_path,
                        usb_path: &mut self.usb_path,
                        mounted_file_path: &mut self.mounted_file_path,
                        rom_name: &mut self.rom_name,
                        sys_clock_freq: self.sys_clock_freq,
                        context: self.context,
                    };
                    browser::run_internal_menu(&mut ctx).await
                }
                PageState::Settings => {
                    settings::run_settings_menu(&mut self.display, &mut self.internal, self.config, &self.context)
                        .await
                }
                PageState::DeviceIdSelect => {
                    settings::run_device_id_menu(&mut self.display, &mut self.internal, self.config, &self.context)
                        .await
                }
                PageState::BootRomSelect => {
                    settings::run_select_boot_rom_menu(
                        &mut self.display,
                        &mut self.internal,
                        self.config,
                        &mut self.rom_name,
                        &self.context,
                    )
                    .await
                }
            }
        }
    }
}
