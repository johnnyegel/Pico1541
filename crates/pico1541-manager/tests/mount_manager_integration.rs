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

#[no_mangle]
fn _embassy_time_now() -> u64 {
    0
}

#[no_mangle]
fn _embassy_time_schedule_wake(_at: u64, _waker: &core::task::Waker) {}

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
        _callback: impl FnMut(DirectoryEntry),
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
    static CMD_CH: Channel<CriticalSectionRawMutex, InternalCommand, 128> = Channel::new();

    let mut controller = MountControllerTask::new(
        MockLeds::new(),
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
