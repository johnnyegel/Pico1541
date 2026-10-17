use crate::flash_storage::FlashStorage;
use littlefs_rust::{Config, Filesystem, OpenFlags};
use pico1541_manager::SettingsCommand;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Receiver;

#[derive(Clone, Copy)]
pub struct RomBufferPtr(pub *mut [u8; 16384]);
unsafe impl Send for RomBufferPtr {}

#[embassy_executor::task]
pub async fn settings_save_task(
    mut flash_storage: FlashStorage<'static>,
    rom_buffer: RomBufferPtr,
    snooper_config: &'static pico1541_common::snooper::SnooperConfig,
    rx: Receiver<'static, CriticalSectionRawMutex, SettingsCommand, 2>,
) {
    loop {
        let cmd = rx.receive().await;

        let mut config = Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;

        snooper_config
            .halt_emulator
            .store(true, portable_atomic::Ordering::Relaxed);

        // Wait for emulation loop to completely stop memory accesses
        embassy_time::Timer::after(embassy_time::Duration::from_millis(50)).await;

        match cmd {
            SettingsCommand::SaveDriveId(id) => match Filesystem::mount(flash_storage, config) {
                Ok(fs) => {
                    if let Ok(file) = fs.open(
                        "/settings.dat",
                        OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC,
                    ) {
                        let _ = file.write(&[id]);
                        let _ = file.sync();
                        defmt::info!("Successfully wrote /settings.dat: Drive ID {}", id);
                    }
                    flash_storage = fs.unmount().unwrap();
                }
                Err((_, storage)) => {
                    flash_storage = storage;
                }
            },
            SettingsCommand::SaveBootRom(name) => match Filesystem::mount(flash_storage, config) {
                Ok(fs) => {
                    let _ = fs.mkdir("/settings");
                    if let Ok(file) = fs.open(
                        "/settings/boot_rom.txt",
                        OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC,
                    ) {
                        let _ = file.write(name.as_bytes());
                        let _ = file.sync();
                        defmt::info!(
                            "Successfully wrote /settings/boot_rom.txt: {}",
                            name.as_str()
                        );
                    }
                    flash_storage = fs.unmount().unwrap();
                }
                Err((_, storage)) => {
                    flash_storage = storage;
                }
            },
            SettingsCommand::DeleteRom(name) => match Filesystem::mount(flash_storage, config) {
                Ok(fs) => {
                    let mut full_path = heapless::String::<128>::new();
                    let _ = full_path.push_str("/roms/");
                    let _ = full_path.push_str(name.as_str());
                    let _ = fs.remove(full_path.as_str());
                    defmt::info!("Deleted ROM: {}", full_path.as_str());
                    flash_storage = fs.unmount().unwrap();
                }
                Err((_, storage)) => {
                    flash_storage = storage;
                }
            },
            SettingsCommand::InstallRomStart { filename, .. } => {
                match Filesystem::mount(flash_storage, config) {
                    Ok(fs) => {
                        let _ = fs.mkdir("/roms");
                        let mut full_path = heapless::String::<128>::new();
                        let _ = full_path.push_str("/roms/");
                        let _ = full_path.push_str(filename.as_str());

                        if let Ok(file) = fs.open(
                            full_path.as_str(),
                            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC,
                        ) {
                            defmt::info!("Opened {} for writing", full_path.as_str());
                            loop {
                                let inner_cmd = rx.receive().await;
                                match inner_cmd {
                                    SettingsCommand::InstallRomChunk(data) => {
                                        let _ = file.write(&data);
                                    }
                                    SettingsCommand::InstallRomFinish => {
                                        let _ = file.sync();
                                        defmt::info!("Finished installing ROM");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        flash_storage = fs.unmount().unwrap();
                    }
                    Err((_, storage)) => {
                        flash_storage = storage;
                    }
                }
            }
            SettingsCommand::LoadRomToMemory(name) => {
                match Filesystem::mount(flash_storage, config) {
                    Ok(fs) => {
                        let mut full_path = heapless::String::<128>::new();
                        let _ = full_path.push_str("/roms/");
                        let _ = full_path.push_str(name.as_str());
                        if let Ok(file) = fs.open(full_path.as_str(), OpenFlags::READ) {
                            unsafe {
                                let buf = &mut *rom_buffer.0;
                                let mut bytes_read = 0;
                                while bytes_read < 16384 {
                                    if let Ok(n) = file.read(&mut buf[bytes_read..]) {
                                        if n == 0 {
                                            break;
                                        }
                                        bytes_read += n as usize;
                                    } else {
                                        break;
                                    }
                                }
                            }
                            defmt::info!("Loaded ROM {} into memory", name.as_str());
                            snooper_config
                                .trigger_reset
                                .store(true, portable_atomic::Ordering::Relaxed);
                        }
                        flash_storage = fs.unmount().unwrap();
                    }
                    Err((_, storage)) => {
                        flash_storage = storage;
                    }
                }
            }
            SettingsCommand::SaveSoundSettings(settings) => {
                crate::tasks::sound::CURRENT_SOUND_SETTINGS.set(settings);
                match Filesystem::mount(flash_storage, config) {
                    Ok(fs) => {
                        let _ = fs.mkdir("/settings");
                        if let Ok(file) = fs.open(
                            "/settings/sound.dat",
                            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC,
                        ) {
                            let bytes = settings.to_bytes();
                            let _ = file.write(&bytes);
                            let _ = file.sync();
                            defmt::info!("Successfully wrote /settings/sound.dat: {:?}", settings);
                        }
                        flash_storage = fs.unmount().unwrap();
                    }
                    Err((_, storage)) => {
                        flash_storage = storage;
                    }
                }
            }
            _ => {}
        }

        snooper_config
            .halt_emulator
            .store(false, portable_atomic::Ordering::Relaxed);
    }
}
