use core::sync::atomic::AtomicBool;
use embassy_futures::block_on;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use pico1541_common::interface::TrackHandoff;
use pico1541_common::track_buffer::DualCoreTrackBuffer;
use pico1541_common::DriveLocation;
use pico1541_manager::driver::{RgbLedDriver, StatusLedMode};
use pico1541_manager::formats::Error as FormatError;
use pico1541_manager::storage::{DirectoryEntry, MemoryStorage};
use pico1541_manager::tasks::mount_manager::{
    FileOpener, MountControllerTask, TrackCacheTask, CURRENT_PAGE, PAGE_CHANGED,
};
use pico1541_manager::{InternalCommand, MountPayload, LOCATION_CHANGED};

static TIME_NOW: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

#[no_mangle]
fn _embassy_time_now() -> u64 {
    TIME_NOW.load(core::sync::atomic::Ordering::Acquire)
}

#[no_mangle]
fn _embassy_time_schedule_wake(at: u64, waker: &core::task::Waker) {
    TIME_NOW.fetch_max(at, core::sync::atomic::Ordering::AcqRel);
    waker.wake_by_ref();
}

static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct MockLeds {
    mode: StatusLedMode,
}

impl MockLeds {
    fn new() -> Self {
        Self {
            mode: StatusLedMode::Unmounted,
        }
    }
}

impl RgbLedDriver for MockLeds {
    fn set_color(&mut self, _r: bool, _g: bool, _b: bool) {}
    fn set_status(&mut self, mode: StatusLedMode) {
        self.mode = mode;
    }
}

struct DummyOpener;

impl FileOpener for DummyOpener {
    type FileStorage = MemoryStorage<'static>;

    async fn open_file(&self, _requested_petscii: &[u8]) -> Result<Self::FileStorage, FormatError> {
        Err(FormatError::FileNotFound)
    }

    fn open_sd_file(&self, _path: &str) -> Self::FileStorage {
        MemoryStorage::new(&[])
    }

    fn open_internal_file(&self, _path: &str) -> Self::FileStorage {
        MemoryStorage::new(&[])
    }

    async fn list_dir_range(
        &self,
        _path: &str,
        _use_internal: bool,
        _start_index: usize,
        _count: usize,
        _callback: impl FnMut(DirectoryEntry, &str),
    ) -> Result<(), FormatError> {
        Ok(())
    }
}

struct DummyHandoff;

impl TrackHandoff for DummyHandoff {
    fn notify_track_change(&mut self, _half_track: u8) {}
    fn current_track(&self) -> u8 {
        34
    }
    async fn wait_for_change(&mut self) -> u8 {
        34
    }
}

#[test]
fn test_mount_controller_commands() {
    let _lock = TEST_MUTEX.lock().unwrap();
    LOCATION_CHANGED.reset();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);
    static CMD_CH: Channel<CriticalSectionRawMutex, InternalCommand, 32> = Channel::new();

    let mut controller = MountControllerTask::new(
        &LOCATION,
        &MOTOR,
        CMD_CH.receiver(),
    );

    block_on(async {
        let runner = controller.run();
        let test_logic = async {
            // 1. CD:INTERNAL from Root
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:INTERNAL").unwrap(),
                ))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(
                *LOCATION.lock().await,
                DriveLocation::VirtualDirectory {
                    path: heapless::String::try_from("/files").unwrap(),
                    use_internal: true,
                }
            );

            // 2. CD:.. from /files -> Root
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:..").unwrap(),
                ))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(*LOCATION.lock().await, DriveLocation::Root);

            // 3. CD:SD CARD from Root -> SD /
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:SD CARD").unwrap(),
                ))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(
                *LOCATION.lock().await,
                DriveLocation::VirtualDirectory {
                    path: heapless::String::try_from("/").unwrap(),
                    use_internal: false,
                }
            );

            // 4. CD:GAME.D64 from SD / -> DiskImage
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:GAME.D64").unwrap(),
                ))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(
                *LOCATION.lock().await,
                DriveLocation::DiskImage {
                    path: heapless::String::try_from("/GAME.D64").unwrap(),
                    use_internal: false,
                }
            );

            // 5. Mount payload
            CMD_CH
                .send(InternalCommand::Mount(MountPayload::D64(
                    heapless::String::try_from("/OTHER.D64").unwrap(),
                    false,
                )))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(
                *LOCATION.lock().await,
                DriveLocation::DiskImage {
                    path: heapless::String::try_from("/OTHER.D64").unwrap(),
                    use_internal: false,
                }
            );

            // 6. Unmount
            CMD_CH.send(InternalCommand::Unmount).await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(*LOCATION.lock().await, DriveLocation::Root);

            // 7. Page navigation
            *LOCATION.lock().await = DriveLocation::VirtualDirectory {
                path: heapless::String::try_from("/").unwrap(),
                use_internal: false,
            };
            LOCATION_CHANGED.reset();
            PAGE_CHANGED.store(false, core::sync::atomic::Ordering::Release);
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:>>> MORE >>>").unwrap(),
                ))
                .await;
            while !PAGE_CHANGED.load(core::sync::atomic::Ordering::Acquire) {
                embassy_futures::yield_now().await;
            }
            assert_eq!(CURRENT_PAGE.load(core::sync::atomic::Ordering::Acquire), 1);
            assert!(PAGE_CHANGED.load(core::sync::atomic::Ordering::Acquire));

            PAGE_CHANGED.store(false, core::sync::atomic::Ordering::Release);
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:<<< PREV <<<").unwrap(),
                ))
                .await;
            while !PAGE_CHANGED.load(core::sync::atomic::Ordering::Acquire) {
                embassy_futures::yield_now().await;
            }
            assert_eq!(CURRENT_PAGE.load(core::sync::atomic::Ordering::Acquire), 0);
            assert!(PAGE_CHANGED.load(core::sync::atomic::Ordering::Acquire));

            // Verify ACTIVE_CHILDREN capacity is 144
            {
                let guard = pico1541_manager::tasks::mount_manager::ACTIVE_CHILDREN.lock().await;
                assert_eq!(guard.capacity(), 144);
            }

            // 8. CD:COMMODORE_64_-26 resolving via ACTIVE_CHILDREN to full long directory name
            // Verify resolution works for entries past index 48 (e.g. index 60)
            *LOCATION.lock().await = DriveLocation::VirtualDirectory {
                path: heapless::String::try_from("/").unwrap(),
                use_internal: false,
            };
            {
                let mut guard = pico1541_manager::tasks::mount_manager::ACTIVE_CHILDREN.lock().await;
                guard.clear();
                for i in 0..60 {
                    let mut dummy_name = heapless::String::new();
                    let _ = core::fmt::write(&mut dummy_name, format_args!("file_{:03}.prg", i));
                    guard.push(pico1541_manager::tasks::mount_manager::CachedChildEntry {
                        petscii_name: [0x41; 16],
                        real_name: dummy_name,
                        is_dir: false,
                    }).unwrap();
                }
                let mut mangled_petscii = [0xA0; 16];
                let mangled = pico1541_manager::storage::mangle_filename(b"Commodore_64_Demo_Party_2026");
                for (i, &b) in mangled.iter().enumerate() {
                    mangled_petscii[i] = pico1541_manager::storage::ascii_to_petscii(b);
                }
                let mut real_name = heapless::String::new();
                real_name.push_str("Commodore_64_Demo_Party_2026").unwrap();
                guard.push(pico1541_manager::tasks::mount_manager::CachedChildEntry {
                    petscii_name: mangled_petscii,
                    real_name,
                    is_dir: true,
                }).unwrap();
                assert_eq!(guard.len(), 61);
            }
            CMD_CH
                .send(InternalCommand::Cd(
                    heapless::String::try_from("CD:COMMODORE_64_-26").unwrap(),
                ))
                .await;
            LOCATION_CHANGED.wait().await;
            assert_eq!(
                *LOCATION.lock().await,
                DriveLocation::VirtualDirectory {
                    path: heapless::String::try_from("/Commodore_64_Demo_Party_2026").unwrap(),
                    use_internal: false,
                }
            );
        };

        match select(runner, test_logic).await {
            Either::First(_) => panic!("Runner exited unexpectedly"),
            Either::Second(_) => {}
        }
    });
}

#[test]
fn test_track_cache_select_virtual_file_root_slots() {
    let _lock = TEST_MUTEX.lock().unwrap();
    LOCATION_CHANGED.reset();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, _) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    block_on(async {
        LOCATION_CHANGED.reset();
        // Slot 0 at Root selects INTERNAL (/files)
        task.select_virtual_file(0).await;
        assert_eq!(
            *LOCATION.lock().await,
            DriveLocation::VirtualDirectory {
                path: heapless::String::try_from("/files").unwrap(),
                use_internal: true,
            }
        );
        assert!(LOCATION_CHANGED.signaled());
        LOCATION_CHANGED.reset();

        // Reset to Root
        *LOCATION.lock().await = DriveLocation::Root;

        // Slot 1 at Root selects SD CARD (/)
        task.select_virtual_file(1).await;
        assert_eq!(
            *LOCATION.lock().await,
            DriveLocation::VirtualDirectory {
                path: heapless::String::try_from("/").unwrap(),
                use_internal: false,
            }
        );
        assert!(LOCATION_CHANGED.signaled());
        LOCATION_CHANGED.reset();
    });
}

#[test]
fn test_track_cache_select_virtual_file_subdirectory() {
    let _lock = TEST_MUTEX.lock().unwrap();
    LOCATION_CHANGED.reset();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::VirtualDirectory {
            path: heapless::String::new(),
            use_internal: false,
        });
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, _) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    // Populate a virtual disk with a directory entry "GAMES"
    let mut dir_source = pico1541_manager::storage::PagedDirectorySource::<144>::new(b"TEST");
    dir_source.set_page_entry(0, DirectoryEntry::new_dir(b"GAMES", 17, 0));
    dir_source.set_total_count(1);
    let vdisk = pico1541_manager::formats::VirtualDiskImage::new(dir_source, b"TEST");
    task.disk_image = Some(pico1541_manager::formats::AnyDiskImage::VirtualDisk(vdisk));

    block_on(async {
        *LOCATION.lock().await = DriveLocation::VirtualDirectory {
            path: heapless::String::try_from("/").unwrap(),
            use_internal: false,
        };
        LOCATION_CHANGED.reset();

        task.select_virtual_file(0).await;

        assert_eq!(
            *LOCATION.lock().await,
            DriveLocation::VirtualDirectory {
                path: heapless::String::try_from("/GAMES").unwrap(),
                use_internal: false,
            }
        );
        assert!(LOCATION_CHANGED.signaled());
    });
}

#[test]
fn test_resolve_child_in_memory_mangled_long_directory() {
    let _lock = TEST_MUTEX.lock().unwrap();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, _) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    let mut dir_source = pico1541_manager::storage::PagedDirectorySource::<144>::new(b"TEST");
    let full_name = "Commodore_64_Demo_Party_2026";
    let mangled = pico1541_manager::storage::mangle_filename(full_name.as_bytes());
    dir_source.set_page_entry_with_name(
        0,
        DirectoryEntry::new_dir(&mangled, 17, 0),
        full_name,
    );
    dir_source.set_total_count(1);
    let vdisk = pico1541_manager::formats::VirtualDiskImage::new(dir_source, b"TEST");
    task.disk_image = Some(pico1541_manager::formats::AnyDiskImage::VirtualDisk(vdisk));

    let resolved = task.resolve_child_in_memory("COMMODORE_64_-26");

    assert_eq!(
        resolved,
        Some((
            heapless::String::try_from("Commodore_64_Demo_Party_2026").unwrap(),
            true
        ))
    );
}

#[test]
fn test_resolve_child_in_memory_mixed_case_d64_image() {
    let _lock = TEST_MUTEX.lock().unwrap();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, _) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    let mut dir_source = pico1541_manager::storage::PagedDirectorySource::<144>::new(b"TEST");
    let full_name = "Disk1.d64";
    let mangled = pico1541_manager::storage::mangle_filename(full_name.as_bytes());
    dir_source.set_page_entry_with_name(
        0,
        DirectoryEntry::new(&mangled, 17, 0, 664),
        full_name,
    );
    dir_source.set_total_count(1);
    let vdisk = pico1541_manager::formats::VirtualDiskImage::new(dir_source, b"TEST");
    task.disk_image = Some(pico1541_manager::formats::AnyDiskImage::VirtualDisk(vdisk));

    let resolved = task.resolve_child_in_memory("DISK1.D64");

    assert_eq!(
        resolved,
        Some((
            heapless::String::try_from("Disk1.d64").unwrap(),
            false
        ))
    );
}

#[test]
fn test_simulate_disk_swap_timing_and_layout() {
    let _lock = TEST_MUTEX.lock().unwrap();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, emulator_handle) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    block_on(async {
        // Test simulate_disk_swap to final_write_protected = false
        task.simulate_disk_swap(false).await;
        assert_eq!(
            emulator_handle.get_active_track_buffer().layout,
            pico1541_common::track_buffer::TrackLayout::Void
        );
        assert!(!emulator_handle.is_write_protected());

        // Test simulate_disk_swap to final_write_protected = true
        task.simulate_disk_swap(true).await;
        assert_eq!(
            emulator_handle.get_active_track_buffer().layout,
            pico1541_common::track_buffer::TrackLayout::Void
        );
        assert!(emulator_handle.is_write_protected());
    });
}

#[test]
fn test_mount_sequence_d64_does_not_halt_emulator() {
    let _lock = TEST_MUTEX.lock().unwrap();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    static D64_DATA: [u8; 174848] = [0u8; 174848];

    struct D64Opener;
    impl FileOpener for D64Opener {
        type FileStorage = MemoryStorage<'static>;
        async fn open_file(&self, _petscii: &[u8]) -> Result<Self::FileStorage, FormatError> {
            Err(FormatError::FileNotFound)
        }
        fn open_sd_file(&self, _path: &str) -> Self::FileStorage {
            assert!(
                !pico1541_common::HALT_EMULATOR.load(core::sync::atomic::Ordering::Acquire),
                "HALT_EMULATOR must be false during mount_sequence for real disk images"
            );
            MemoryStorage::new(&D64_DATA)
        }
        fn open_internal_file(&self, _path: &str) -> Self::FileStorage {
            assert!(
                !pico1541_common::HALT_EMULATOR.load(core::sync::atomic::Ordering::Acquire),
                "HALT_EMULATOR must be false during mount_sequence for real disk images"
            );
            MemoryStorage::new(&D64_DATA)
        }
        async fn list_dir_range(
            &self,
            _path: &str,
            _use_internal: bool,
            _start_index: usize,
            _count: usize,
            _callback: impl FnMut(DirectoryEntry, &str),
        ) -> Result<(), FormatError> {
            Ok(())
        }
    }

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, emulator_handle) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        D64Opener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    block_on(async {
        // Simulating entry from CD:image.d64 where Core 0 was halted by CBM DOS command interception
        pico1541_common::HALT_EMULATOR.store(true, core::sync::atomic::Ordering::Release);

        task.mount_sequence(heapless::String::try_from("/GAME.D64").unwrap(), false).await;

        // Verify HALT_EMULATOR is false during and after the mount
        assert!(
            !pico1541_common::HALT_EMULATOR.load(core::sync::atomic::Ordering::Acquire),
            "HALT_EMULATOR must be false after mounting real D64 images even if previously halted"
        );

        // Verify final write protected state is false for real disk image
        assert!(
            !emulator_handle.is_write_protected(),
            "D64 disk image must default to write-enabled (not protected)"
        );

        // Verify active layout was loaded from the D64 image (not Void)
        assert_ne!(
            emulator_handle.get_active_track_buffer().layout,
            pico1541_common::track_buffer::TrackLayout::Void,
            "Mounted D64 must transition from Void to active track data layout"
        );
    });
}

#[test]
fn test_mount_sequence_virtual_dir_write_protected() {
    let _lock = TEST_MUTEX.lock().unwrap();
    static LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
        Mutex::new(DriveLocation::Root);
    static MOTOR: AtomicBool = AtomicBool::new(false);

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buf)));
    let (manager_handle, emulator_handle) = buffer.split();

    let mut task = TrackCacheTask::new(
        manager_handle,
        DummyOpener,
        DummyHandoff,
        &LOCATION,
        &MOTOR,
        MockLeds::new(),
    );

    block_on(async {
        pico1541_common::HALT_EMULATOR.store(false, core::sync::atomic::Ordering::Release);

        task.mount_sequence(heapless::String::try_from("/GAMES").unwrap(), false).await;

        // Virtual directory must end with HALT_EMULATOR restored to false
        assert!(
            !pico1541_common::HALT_EMULATOR.load(core::sync::atomic::Ordering::Acquire),
            "HALT_EMULATOR must be unhalted after virtual directory mount completes"
        );

        // Virtual directory must be write-protected
        assert!(
            emulator_handle.is_write_protected(),
            "Virtual directory must be write-protected"
        );
    });
}

