use futures_executor::block_on;
use pico1541_manager::{D64Image, DiskImage, MemoryStorage};

#[test]
fn test_d64_load_track() {
    // Create a dummy D64 with some data
    let mut d64_data = [0u8; 174848];
    // Track 1 Sector 0 (offset 0)
    d64_data[0] = 0xA2; // LDX #imm
    d64_data[1] = 0x10;

    let storage = MemoryStorage::new(&d64_data);
    let mut image = block_on(D64Image::new(storage));

    use pico1541_common::track_buffer::TrackLayout;
    let mut buffer = [0u8; 10000];
    let layout = block_on(image.read_track(2, &mut buffer)).unwrap(); // Track 1.0 = Half-track 2

    let bit_len = match layout.0 {
        TrackLayout::Accurate | TrackLayout::Warped(_) => layout.1,
        _ => panic!("Expected Accurate or Warped track"),
    };

    assert!(bit_len > 0);

    // Check if we can find the sync mark ($FF x 8)
    let mut found_sync = false;
    for window in buffer.windows(5) {
        if window == [0xFF; 5] {
            found_sync = true;
            break;
        }
    }
    assert!(found_sync, "Sync mark not found in GCR");
}

#[test]
fn test_gcr_encoding() {
    use pico1541_manager::gcr;
    let data = [0x01, 0x02, 0x03, 0x04];
    let mut gcr_buf = [0u8; 5];
    gcr::encode_gcr_block(&data, &mut gcr_buf);

    let mut decoded = [0u8; 4];
    gcr::decode_gcr_block(&gcr_buf, &mut decoded);

    assert_eq!(data, decoded);
}

#[test]
fn test_get_status_mode() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::StaticDirectorySource;
    use pico1541_manager::{
        get_status_mode, AnyDiskImage, D64Image, G64Image, MemoryStorage, StatusLedMode,
    };

    // 1. None (Unmounted)
    assert_eq!(
        get_status_mode::<MemoryStorage, StaticDirectorySource>(None),
        StatusLedMode::Unmounted
    );

    // 2. VirtualDisk
    let dir_source = StaticDirectorySource::new(b"TEST", &[]);
    let vdisk = VirtualDiskImage::new(dir_source, b"TEST");
    let disk: AnyDiskImage<MemoryStorage, StaticDirectorySource> = AnyDiskImage::VirtualDisk(vdisk);
    assert_eq!(get_status_mode(Some(&disk)), StatusLedMode::VirtualDisk);

    // 3. D64 (Non-accurate)
    let d64_data = [0u8; 174848];
    let d64 = block_on(D64Image::new(MemoryStorage::new(&d64_data)));
    let disk: AnyDiskImage<MemoryStorage, StaticDirectorySource> = AnyDiskImage::D64(d64);
    assert_eq!(get_status_mode(Some(&disk)), StatusLedMode::NonAccurateDisk);

    // 4. G64 (Accurate)
    let g64_data = [0u8; 1000];
    let g64 = block_on(G64Image::from_storage(MemoryStorage::new(&g64_data))).unwrap();
    let disk: AnyDiskImage<MemoryStorage, StaticDirectorySource> = AnyDiskImage::G64(g64);
    assert_eq!(get_status_mode(Some(&disk)), StatusLedMode::AccurateDisk);
}

#[test]
fn test_mount_payload_g64() {
    use core::str::FromStr;
    use pico1541_manager::{MountPayload, StatusLedMode};

    let path = heapless::String::<128>::from_str("/games/test.g64").unwrap();

    // 1. G64 on internal storage
    let payload_internal = MountPayload::G64(path.clone(), true);
    assert_eq!(payload_internal.path(), "/games/test.g64");
    assert_eq!(payload_internal.use_internal(), true);
    assert_eq!(payload_internal.status_led_mode(), StatusLedMode::AccurateDisk);

    let (p, internal, mode) = payload_internal.into_parts();
    assert_eq!(p.as_str(), "/games/test.g64");
    assert_eq!(internal, true);
    assert_eq!(mode, StatusLedMode::AccurateDisk);

    // 2. G64 on SD storage
    let payload_sd = MountPayload::G64(path.clone(), false);
    assert_eq!(payload_sd.path(), "/games/test.g64");
    assert_eq!(payload_sd.use_internal(), false);
    assert_eq!(payload_sd.status_led_mode(), StatusLedMode::AccurateDisk);

    let (p, internal, mode) = payload_sd.into_parts();
    assert_eq!(p.as_str(), "/games/test.g64");
    assert_eq!(internal, false);
    assert_eq!(mode, StatusLedMode::AccurateDisk);
}

