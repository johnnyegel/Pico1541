use pico1541_common::track_buffer::TrackLayout;
use pico1541_manager::formats::{D64Image, DiskImage, G64Image, NibImage, VirtualDiskImage};
use pico1541_manager::storage::{DirectoryEntry, DirectorySource, MemoryStorage};
use pollster::block_on;

#[test]
fn test_d64_image_read_track() {
    let mut data = vec![0u8; 174848];
    // Fill track 1 sector 0 with some recognizable data
    let t1s0_offset = 0;
    for i in 0..256 {
        data[t1s0_offset + i] = i as u8;
    }

    let storage = MemoryStorage::new(&data);
    let mut img = block_on(D64Image::new(storage));
    let mut dest = vec![0u8; 8000];

    // Track 1.0 is half_track 2
    let (layout, len, _) = block_on(img.read_track(2, &mut dest)).unwrap();
    assert!(matches!(layout, TrackLayout::Accurate));
    assert!(len > 7000);

    // Check if we can find the GCR encoded sector 0
    assert!(dest.iter().any(|&b| b != 0));
}

#[test]
fn test_g64_image_read_track() {
    let mut data = vec![0u8; 1024];
    data[0..8].copy_from_slice(b"GCR-1541");
    data[8] = 0; // version
    data[9] = 42; // tracks
    data[10..12].copy_from_slice(&64u16.to_le_bytes()); // max track length

    // Track 1.0 (HT 0) pointer at 12
    let track_offset = 100u32;
    data[12..16].copy_from_slice(&track_offset.to_le_bytes());

    // Track data at offset 100
    let track_len = 50u16;
    data[100..102].copy_from_slice(&track_len.to_le_bytes());
    for i in 0..track_len as usize {
        data[102 + i] = 0x55;
    }

    let storage = MemoryStorage::new(&data);
    let mut img = block_on(G64Image::from_storage(storage)).unwrap();
    let mut dest = vec![0u8; 100];

    let result = block_on(img.read_track(0, &mut dest)).unwrap();
    assert!(matches!(result, (TrackLayout::Accurate, 50, _)));
    assert_eq!(dest[0], 0x55);
}

#[test]
fn test_virtual_disk_image() {
    struct MockSource {
        entries: Vec<DirectoryEntry>,
        pos: usize,
    }
    impl DirectorySource for MockSource {
        fn name(&self) -> &[u8; 16] {
            b"MOCK DISK       "
        }
        fn next_entry(&mut self) -> Option<DirectoryEntry> {
            if self.pos < self.entries.len() {
                let entry = self.entries[self.pos];
                self.pos += 1;
                Some(entry)
            } else {
                None
            }
        }
        fn rewind(&mut self) {
            self.pos = 0;
        }
    }

    let source = MockSource {
        entries: vec![DirectoryEntry::new(b"FILE1", 1, 0, 10)],
        pos: 0,
    };
    let mut img = VirtualDiskImage::<MemoryStorage<'static>, MockSource>::new(source, b"TEST");
    let mut dest = vec![0u8; 10000];

    let (layout, len, _) = block_on(img.read_track(36, &mut dest)).unwrap();
    assert!(matches!(layout, TrackLayout::Accurate));
    assert!(len > 7000);
    assert!(dest.iter().any(|&b| b != 0));
}

#[test]
fn test_nib_image_read_track() {
    let mut data = vec![0u8; 0x100 + 8192];
    // Header magic
    data[0..13].copy_from_slice(b"MNIB-1541-RAW");
    data[13] = 2; // version
    // Track entry 0 at 0x10: Track 18.0 (track_num = 36), density = 2 (speed zone 2)
    data[0x10] = 36;
    data[0x11] = 0x02;
    // Track payload at 0x100
    for i in 0..8192 {
        data[0x100 + i] = 0x55;
    }
    // Place a repeating header at 50 and 50 + 7132 = 7182
    data[0x100 + 50..0x100 + 52].copy_from_slice(&[0xFF, 0xFF]);
    data[0x100 + 52] = 0x52;
    data[0x100 + 53..0x100 + 62].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);

    data[0x100 + 7182..0x100 + 7184].copy_from_slice(&[0xFF, 0xFF]);
    data[0x100 + 7184] = 0x52;
    data[0x100 + 7185..0x100 + 7194].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);

    let storage = MemoryStorage::new(&data);
    let mut nib = block_on(NibImage::from_storage(storage)).expect("failed to load NIB");

    let mut dest = vec![0u8; 8192];
    // Track 18.0 corresponds to half_track 34 in Pico1541
    let (layout, len, speed) = block_on(nib.read_track(34, &mut dest)).expect("read_track failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert_eq!(len, 7132);
    assert_eq!(speed, Some(2));
    assert_eq!(&dest[50..53], &[0xFF, 0xFF, 0x52]);

    // Unmapped track should return Void
    let (layout, len, speed) = block_on(nib.read_track(0, &mut dest)).expect("unmapped failed");
    assert_eq!(layout, TrackLayout::Void);
    assert_eq!(len, 0);
    assert_eq!(speed, None);
}

#[test]
fn test_nib_image_1571_magic() {
    let mut data = vec![0u8; 0x100];
    data[0..13].copy_from_slice(b"MNIB-1571-RAW");
    let storage = MemoryStorage::new(&data);
    let nib = block_on(NibImage::from_storage(storage));
    assert!(nib.is_ok());
}

#[test]
fn test_nib_image_invalid_magic() {
    let mut data = vec![0u8; 0x100];
    data[0..13].copy_from_slice(b"BAD-MAGIC-RAW");
    let storage = MemoryStorage::new(&data);
    let nib = block_on(NibImage::from_storage(storage));
    assert!(nib.is_err());
}

#[test]
fn test_nib_image_dest_too_small() {
    let mut data = vec![0u8; 0x100 + 8192];
    data[0..13].copy_from_slice(b"MNIB-1541-RAW");
    data[0x10] = 36;
    data[0x11] = 0x02;
    let storage = MemoryStorage::new(&data);
    let mut nib = block_on(NibImage::from_storage(storage)).unwrap();
    let mut small_dest = vec![0u8; 4096];
    let res = block_on(nib.read_track(34, &mut small_dest));
    assert!(matches!(res, Err(pico1541_manager::formats::Error::TrackTooLarge)));
}

#[test]
fn test_any_disk_image_nib() {
    let mut data = vec![0u8; 0x100 + 8192];
    data[0..13].copy_from_slice(b"MNIB-1541-RAW");
    data[0x10] = 36;
    data[0x11] = 0x02;
    for i in 0..8192 {
        data[0x100 + i] = 0x42;
    }

    struct DummyDir;
    impl DirectorySource for DummyDir {
        fn name(&self) -> &[u8; 16] {
            b"                "
        }
        fn next_entry(&mut self) -> Option<DirectoryEntry> {
            None
        }
        fn rewind(&mut self) {}
    }

    let storage = MemoryStorage::new(&data);
    let nib = block_on(NibImage::from_storage(storage)).unwrap();
    let mut any_disk: pico1541_manager::formats::AnyDiskImage<MemoryStorage, DummyDir> =
        pico1541_manager::formats::AnyDiskImage::Nib(nib);

    let mut dest = vec![0u8; 8192];
    let (layout, len, speed) = block_on(any_disk.read_track(34, &mut dest)).unwrap();
    assert_eq!(layout, TrackLayout::Accurate);
    // Unformatted dummy data falls back to nominal capacity for Zone 2 (7142)
    assert_eq!(len, 7142);
    assert_eq!(speed, Some(2));
    assert_eq!(dest[0], 0x42);
    assert!(any_disk.as_virtual_disk_mut().is_none());
}

#[test]
fn test_nib_image_multi_track() {
    let mut data = vec![0u8; 0x100 + 8192 * 2];
    // Header magic
    data[0..13].copy_from_slice(b"MNIB-1541-RAW");
    data[13] = 2; // version

    // Track entry 0 at 0x10: Track 1.0 (track_num = 2), density = 3 (speed zone 3)
    data[0x10] = 2;
    data[0x11] = 0x03;

    // Track entry 1 at 0x12: Track 18.0 (track_num = 36), density = 1 (speed zone 1)
    data[0x12] = 36;
    data[0x13] = 0x01;

    // Track 1 payload at 0x100 (t_index = 0)
    for i in 0..8192 {
        data[0x100 + i] = 0x11;
    }

    // Track 18 payload at 0x2100 (t_index = 1)
    for i in 0..8192 {
        data[0x2100 + i] = 0x22;
    }

    let storage = MemoryStorage::new(&data);
    let mut nib = block_on(NibImage::from_storage(storage)).expect("failed to load NIB");

    let mut dest = vec![0u8; 8192];

    // Read Track 1.0 (half_track = 0) - fallback to nominal capacity for Zone 3 (7692)
    let (layout, len, speed) = block_on(nib.read_track(0, &mut dest)).expect("read track 0 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert_eq!(len, 7692);
    assert_eq!(speed, Some(3));
    assert!(dest[..len].iter().all(|&b| b == 0x11));

    // Read Track 18.0 (half_track = 34) - fallback to nominal capacity for Zone 1 (6666)
    let (layout, len, speed) = block_on(nib.read_track(34, &mut dest)).expect("read track 34 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert_eq!(len, 6666);
    assert_eq!(speed, Some(1));
    assert!(dest[..len].iter().all(|&b| b == 0x22));

    // Read unmapped Track 2.0 (half_track = 2)
    let (layout, len, speed) = block_on(nib.read_track(2, &mut dest)).expect("read unmapped failed");
    assert_eq!(layout, TrackLayout::Void);
    assert_eq!(len, 0);
    assert_eq!(speed, None);
}

#[test]
fn test_nib_image_metadata_in_header_ignored() {
    let mut data = vec![0u8; 0x100 + 8192];
    data[0..13].copy_from_slice(b"MNIB-1541-RAW");
    // One track at 0x10
    data[0x10] = 2; // track 1.0 -> half_track 0
    data[0x11] = 0; // speed zone 0
    // Non-zero metadata in bytes 0xB8..0xFF (fill with 0xFF)
    for b in &mut data[0xB8..0x100] {
        *b = 0xFF;
    }
    let storage = MemoryStorage::new(&data);
    let mut nib = block_on(NibImage::from_storage(storage)).expect("failed to load NIB");
    let mut dest = vec![0u8; 8192];
    let (layout, len, _) = block_on(nib.read_track(0, &mut dest)).expect("read track 0 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    // Zone 0 nominal capacity is 6250
    assert_eq!(len, 6250);
}

#[test]
fn test_directory_entry_file_type_is_dir() {
    let dir_entry = pico1541_manager::storage::DirectoryEntry::from_file_info(b"GAMES", true, 0);
    assert_eq!(
        dir_entry.file_type, 0x86,
        "Directory entries must have file type 0x86 (DIR) for SD2IEC / fb compatibility"
    );

    let prg_entry = pico1541_manager::storage::DirectoryEntry::from_file_info(b"GAME.D64", false, 174848);
    assert_eq!(
        prg_entry.file_type, 0x82,
        "File entries must have file type 0x82 (PRG)"
    );
    assert_eq!(prg_entry.first_track, 17);
    assert_eq!(prg_entry.first_sector, 0);
}

#[test]
fn test_virtual_disk_prg_multitrack() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::DummySource;
    use pico1541_manager::DiskImage;

    // Create 6000 bytes of mock PRG data (spans Track 17 and Track 16)
    let prg_data = vec![0x42u8; 6000];
    let storage = MemoryStorage::new(&prg_data);

    let mut vdisk = VirtualDiskImage::new(DummySource, b"MYGAME.PRG");
    vdisk.set_id([0x31, 0x31]);

    let entry = pico1541_manager::storage::DirectoryEntry::from_file_info(b"MYGAME.PRG", false, 6000);
    vdisk.set_shortcut_entry(Some(entry));
    vdisk.file_storage = Some(storage);

    let mut dest = vec![0u8; 10000];

    // Track 18 (directory)
    let (layout, len, _) = block_on(vdisk.read_track(34, &mut dest)).expect("read track 18 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert!(len > 7000);

    // Track 17 (first PRG track)
    let (layout, len, _) = block_on(vdisk.read_track(32, &mut dest)).expect("read track 17 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert!(len > 7000);

    // Track 16 (second PRG track)
    let (layout, len, _) = block_on(vdisk.read_track(30, &mut dest)).expect("read track 16 failed");
    assert_eq!(layout, TrackLayout::Accurate);
    assert!(len > 7000);
}
