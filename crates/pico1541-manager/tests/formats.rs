use pico1541_common::track_buffer::TrackLayout;
use pico1541_manager::formats::{D64Image, DiskImage, G64Image, VirtualDiskImage};
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

#[test]
fn test_virtual_disk_prg_byte_exact() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::DummySource;
    use pico1541_manager::DiskImage;
    use pico1541_manager::gcr::decode_gcr_block;

    let original_bytes = include_bytes!("../../../flash-fs/files/fb64").to_vec();
    let storage = MemoryStorage::new(&original_bytes);

    let mut vdisk = VirtualDiskImage::new(DummySource, b"FB64");
    vdisk.set_id([0x31, 0x31]);

    let entry = pico1541_manager::storage::DirectoryEntry::from_file_info(b"FB64", false, original_bytes.len() as u32);
    vdisk.set_shortcut_entry(Some(entry));
    vdisk.file_storage = Some(storage);

    let mut dest = vec![0u8; 10000];
    let (layout, len, _) = block_on(vdisk.read_track(32, &mut dest)).expect("read track 17 failed");
    assert_eq!(layout, TrackLayout::Accurate);

    // Decode all sectors from the generated GCR track
    let mut sector_data_map = std::collections::HashMap::new();

    let mut pos = 0;
    while pos + 5 <= len {
        // Find 5 sync bytes
        if dest[pos..pos + 5] == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
            // Header is 10 GCR bytes (2 blocks of 5)
            let mut header_raw = [0u8; 8];
            decode_gcr_block((&dest[pos + 5..pos + 10]).try_into().unwrap(), (&mut header_raw[0..4]).try_into().unwrap());
            decode_gcr_block((&dest[pos + 10..pos + 15]).try_into().unwrap(), (&mut header_raw[4..8]).try_into().unwrap());

            if header_raw[0] == 0x08 {
                let sector = header_raw[2];
                // Advance past header: 5 sync + 10 gcr + 9 gap + 5 data sync = 29 bytes
                let data_gcr_pos = pos + 5 + 10 + 9 + 5;
                if data_gcr_pos + 325 <= len {
                    let mut data_raw = [0u8; 260];
                    for chunk_idx in 0..65 {
                        let src_slice = &dest[data_gcr_pos + chunk_idx * 5..data_gcr_pos + chunk_idx * 5 + 5];
                        let dst_slice = &mut data_raw[chunk_idx * 4..chunk_idx * 4 + 4];
                        decode_gcr_block(src_slice.try_into().unwrap(), dst_slice.try_into().unwrap());
                    }
                    assert_eq!(data_raw[0], 0x07, "Data block mark must be 0x07");
                    let mut sector_buf = [0u8; 256];
                    sector_buf.copy_from_slice(&data_raw[1..257]);
                    sector_data_map.insert(sector, sector_buf);
                    pos = data_gcr_pos + 325;
                    continue;
                }
            }
        }
        pos += 1;
    }

    assert_eq!(sector_data_map.len(), 21, "Must have decoded all 21 sectors");

    // Follow sector chain starting at sector 0
    let mut current_track = 17;
    let mut current_sector = 0;
    let mut reconstructed = Vec::new();

    loop {
        assert_eq!(current_track, 17, "All blocks for fb.prg must be on track 17");
        let sector_buf = sector_data_map.get(&current_sector).expect("sector missing");
        let next_track = sector_buf[0];
        let next_sector = sector_buf[1];

        if next_track == 0 {
            // Last block: next_sector is the last valid byte offset + 1
            let valid_len = (next_sector as usize).saturating_sub(1);
            reconstructed.extend_from_slice(&sector_buf[2..2 + valid_len]);
            break;
        } else {
            // Full block: 254 bytes
            reconstructed.extend_from_slice(&sector_buf[2..256]);
            current_track = next_track;
            current_sector = next_sector;
        }
    }

    assert_eq!(reconstructed.len(), original_bytes.len(), "Reconstructed length must match");
    assert_eq!(reconstructed, original_bytes, "Reconstructed bytes must be byte-for-byte identical to fb.prg");
}

#[test]
fn test_paged_directory_navigation_entries() {
    use pico1541_manager::storage::{
        DirectoryEntry, DirectorySource, PagedDirectorySource, PAGE_CAPACITY,
    };

    let mut source = PagedDirectorySource::<144>::new(b"TEST DIR");
    source.set_total_count(200); // More than 1 page

    for i in 0..144 {
        let name = format!("FILE_{}", i);
        source.entries[i] = DirectoryEntry::new(name.as_bytes(), 17, 0, 10);
    }

    // Page 0: No previous entry, 142 real entries + 1 MORE entry at index 142
    source.set_page(0);
    let mut page0_entries = Vec::new();
    while let Some(entry) = source.next_entry() {
        page0_entries.push(entry);
    }
    assert_eq!(page0_entries.len(), PAGE_CAPACITY + 1); // 143 entries
    assert_ne!(&page0_entries[0].filename[..12], b"<<< PREV <<<");
    assert_eq!(&page0_entries[0].filename[..6], b"FILE_0");
    assert_eq!(&page0_entries[1].filename[..6], b"FILE_1");
    assert_eq!(&page0_entries[PAGE_CAPACITY].filename[..12], b">>> MORE >>>");
    assert_eq!(page0_entries[PAGE_CAPACITY].file_type, 0x86); // DIR

    // Page 1: Starts with "<<< PREV <<<", ends with real entries starting at FILE_0
    source.set_page(1);
    let mut page1_entries = Vec::new();
    while let Some(entry) = source.next_entry() {
        page1_entries.push(entry);
    }
    // Total count is 200: Page 0 has 142 real entries, Page 1 has 200 - 142 = 58 real entries + 1 PREV = 59 entries
    assert_eq!(page1_entries.len(), 59);
    assert_eq!(&page1_entries[0].filename[..12], b"<<< PREV <<<");
    assert_eq!(page1_entries[0].file_type, 0x86); // DIR
    // Assert that page1_entries[1].filename matches FILE_0 (verifying no off-by-one skipping real_slot 0)
    assert_eq!(&page1_entries[1].filename[..6], b"FILE_0");
    assert_eq!(&page1_entries[2].filename[..6], b"FILE_1");
    assert_eq!(&page1_entries[58].filename[..7], b"FILE_57");

    // Page 1 with > 284 entries: should have PREV at 0, 142 real entries, and MORE at 143
    source.set_total_count(300);
    source.set_page(1);
    let mut page1_more_entries = Vec::new();
    while let Some(entry) = source.next_entry() {
        page1_more_entries.push(entry);
    }
    assert_eq!(page1_more_entries.len(), 144);
    assert_eq!(&page1_more_entries[0].filename[..12], b"<<< PREV <<<");
    assert_eq!(&page1_more_entries[1].filename[..6], b"FILE_0");
    assert_eq!(&page1_more_entries[143].filename[..12], b">>> MORE >>>");
    assert_eq!(page1_more_entries[143].file_type, 0x86);
}

#[test]
fn test_directory_entry_unique_track_sector_mapping() {
    use pico1541_manager::storage::{slot_to_track_sector, track_sector_to_slot};

    // Slot 0 -> Track 17, Sector 0
    assert_eq!(slot_to_track_sector(0), (17, 0));
    // Slot 20 -> Track 17, Sector 20
    assert_eq!(slot_to_track_sector(20), (17, 20));
    // Slot 21 -> Track 16, Sector 0
    assert_eq!(slot_to_track_sector(21), (16, 0));
    // Slot 143 -> Track 11, Sector 17
    assert_eq!(slot_to_track_sector(143), (11, 17));
    // Slot 146 -> Track 11, Sector 20
    assert_eq!(slot_to_track_sector(146), (11, 20));

    // Underflow protection: slot >= 147 clamps to track 11
    assert_eq!(slot_to_track_sector(147), (11, 0));
    assert_eq!(slot_to_track_sector(200), (11, 200 % 21));
    assert_eq!(
        slot_to_track_sector(usize::MAX),
        (11, (usize::MAX % 21) as u8)
    );

    // Roundtrip bijection track_sector_to_slot(slot_to_track_sector(s)) for all valid slots 0..146
    for s in 0..=146 {
        let (track, sector) = slot_to_track_sector(s);
        assert!(
            (11..=17).contains(&track),
            "Track {} out of bounds for slot {}",
            track,
            s
        );
        assert!(
            sector < 21,
            "Sector {} out of bounds for slot {}",
            sector,
            s
        );
        let mapped = track_sector_to_slot(track, sector);
        assert_eq!(mapped, Some(s), "Bijection failed for slot {}", s);
    }
}

#[test]
fn test_virtual_disk_static_track_18_all_sectors() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::{DirectoryEntry, VectorDirectorySource};
    use pico1541_manager::DiskImage;
    use pico1541_manager::gcr::decode_gcr_block;
    use pollster::block_on;

    // Create 30 entries (fills Sector 1, 2, 3, and part of 4)
    let mut entries = heapless::Vec::<DirectoryEntry, 144>::new();
    for i in 0..30 {
        let mut name = [b' '; 16];
        let s = format!("FILE{}", i);
        name[..s.len()].copy_from_slice(s.as_bytes());
        let _ = entries.push(DirectoryEntry::new(&name, 17, 0, 10));
    }
    let source = VectorDirectorySource::new(b"TEST DISK", entries);
    let mut vdisk =
        VirtualDiskImage::<pico1541_manager::storage::MemoryStorage<'static>, _>::new(
            source,
            b"TEST DISK",
        );

    let mut dest = vec![0u8; 10000];
    let (layout, len, _) = block_on(vdisk.read_track(34, &mut dest)).unwrap();
    assert_eq!(layout, pico1541_common::track_buffer::TrackLayout::Accurate);

    // Decode all sectors and verify all 19 sectors (0..18) are present
    let mut sector_data_map = std::collections::HashMap::new();
    let mut pos = 0;
    while pos + 5 <= len {
        if dest[pos..pos + 5] == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
            let mut header_raw = [0u8; 8];
            decode_gcr_block(
                (&dest[pos + 5..pos + 10]).try_into().unwrap(),
                (&mut header_raw[0..4]).try_into().unwrap(),
            );
            decode_gcr_block(
                (&dest[pos + 10..pos + 15]).try_into().unwrap(),
                (&mut header_raw[4..8]).try_into().unwrap(),
            );

            if header_raw[0] == 0x08 {
                let sector = header_raw[2];
                let data_gcr_pos = pos + 5 + 10 + 9 + 5;
                if data_gcr_pos + 325 <= len {
                    let mut data_raw = [0u8; 260];
                    for chunk_idx in 0..65 {
                        let src_slice =
                            &dest[data_gcr_pos + chunk_idx * 5..data_gcr_pos + chunk_idx * 5 + 5];
                        let dst_slice = &mut data_raw[chunk_idx * 4..chunk_idx * 4 + 4];
                        decode_gcr_block(
                            src_slice.try_into().unwrap(),
                            dst_slice.try_into().unwrap(),
                        );
                    }
                    let mut sector_buf = [0u8; 256];
                    sector_buf.copy_from_slice(&data_raw[1..257]);
                    sector_data_map.insert(sector, sector_buf);
                    pos = data_gcr_pos + 325;
                    continue;
                }
            }
        }
        pos += 1;
    }

    assert_eq!(
        sector_data_map.len(),
        19,
        "All 19 sectors must be present on Track 18"
    );

    // Sector 0 BAM points to (18, 1)
    let s0 = sector_data_map.get(&0).unwrap();
    assert_eq!(s0[0], 18);
    assert_eq!(s0[1], 1);

    // Sector 1 has 8 entries, points to (18, 2)
    let s1 = sector_data_map.get(&1).unwrap();
    assert_eq!(s1[0], 18);
    assert_eq!(s1[1], 2);

    // Sector 2 has 8 entries, points to (18, 3)
    let s2 = sector_data_map.get(&2).unwrap();
    assert_eq!(s2[0], 18);
    assert_eq!(s2[1], 3);

    // Sector 3 has 8 entries, points to (18, 4)
    let s3 = sector_data_map.get(&3).unwrap();
    assert_eq!(s3[0], 18);
    assert_eq!(s3[1], 4);

    // Sector 4 has 6 entries (30 - 24 = 6), points to (0, 0xFF)
    let s4 = sector_data_map.get(&4).unwrap();
    assert_eq!(s4[0], 0);
    assert_eq!(s4[1], 0xFF);

    for s in 5..=18 {
        let sec = sector_data_map.get(&s).unwrap();
        assert_eq!(sec[0], 0, "Empty sector {} must point to track 0", s);
        assert_eq!(sec[1], 0xFF, "Empty sector {} must point to sector 0xFF", s);
    }
}

#[test]
fn test_resolve_cd_navigation_commands() {
    use pico1541_manager::tasks::mount_manager::cd_handler::{resolve_cd_action, CdAction};

    let h = |s: &str| heapless::String::<128>::try_from(s).unwrap();

    assert_eq!(resolve_cd_action("/", "CD:>>> MORE >>>"), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:..."), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:<<< PREV <<<"), Some(CdAction::PrevPage));
    assert_eq!(resolve_cd_action("/GAMES", "CD:.."), Some(CdAction::ChangeDir(h("/"))));
    assert_eq!(resolve_cd_action("/", "CD:GAMES"), Some(CdAction::ChangeDir(h("/GAMES"))));
    assert_eq!(resolve_cd_action("/", "CD:GAME.D64"), Some(CdAction::MountImage(h("/GAME.D64"))));
    assert_eq!(resolve_cd_action("/", "CD/GAME.G64"), Some(CdAction::MountImage(h("/GAME.G64"))));
}

#[test]
fn test_virtual_disk_prg_multitrack_offset() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::{DummySource, MemoryStorage};
    use pico1541_manager::DiskImage;
    use pico1541_manager::gcr::decode_gcr_block;
    use pollster::block_on;

    let test_data: Vec<u8> = (0..7000).map(|i| (i % 256) as u8).collect();
    let storage = MemoryStorage::new(&test_data);
    let mut vdisk = VirtualDiskImage::new(DummySource, b"OFFSET.PRG");
    vdisk.set_prg_start(16, 5);
    vdisk.file_storage = Some(storage);

    // Track 17 (half track 32) should be Void since track 17 > prg_start_track (16)
    let mut dest17 = vec![0u8; 10000];
    let (l17, _, _) = block_on(vdisk.read_track(32, &mut dest17)).unwrap();
    assert_eq!(l17, pico1541_common::track_buffer::TrackLayout::Void);

    // Read track 16 (half track 30)
    let mut dest16 = vec![0u8; 10000];
    let (l16, len16, _) = block_on(vdisk.read_track(30, &mut dest16)).unwrap();
    assert_eq!(l16, pico1541_common::track_buffer::TrackLayout::Accurate);

    // Read track 15 (half track 28)
    let mut dest15 = vec![0u8; 10000];
    let (l15, len15, _) = block_on(vdisk.read_track(28, &mut dest15)).unwrap();
    assert_eq!(l15, pico1541_common::track_buffer::TrackLayout::Accurate);

    // Helper to decode sectors from track GCR buffer
    let mut sector_data_map = std::collections::HashMap::new();

    for (t, dest, len) in [(16, &dest16, len16), (15, &dest15, len15)] {
        let mut pos = 0;
        while pos + 5 <= len {
            if dest[pos..pos + 5] == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
                let mut header_raw = [0u8; 8];
                decode_gcr_block((&dest[pos + 5..pos + 10]).try_into().unwrap(), (&mut header_raw[0..4]).try_into().unwrap());
                decode_gcr_block((&dest[pos + 10..pos + 15]).try_into().unwrap(), (&mut header_raw[4..8]).try_into().unwrap());

                if header_raw[0] == 0x08 {
                    let sector = header_raw[2];
                    let track = header_raw[3];
                    assert_eq!(track, t);

                    let data_gcr_pos = pos + 5 + 10 + 9 + 5;
                    if data_gcr_pos + 325 <= len {
                        let mut data_raw = [0u8; 260];
                        for chunk_idx in 0..65 {
                            let src_slice = &dest[data_gcr_pos + chunk_idx * 5..data_gcr_pos + chunk_idx * 5 + 5];
                            let dst_slice = &mut data_raw[chunk_idx * 4..chunk_idx * 4 + 4];
                            decode_gcr_block(src_slice.try_into().unwrap(), dst_slice.try_into().unwrap());
                        }
                        assert_eq!(data_raw[0], 0x07, "Data block mark must be 0x07");
                        let mut sector_buf = [0u8; 256];
                        sector_buf.copy_from_slice(&data_raw[1..257]);
                        sector_data_map.insert((track, sector), sector_buf);
                        pos = data_gcr_pos + 325;
                        continue;
                    }
                }
            }
            pos += 1;
        }
    }

    // Decode and verify the sector chain starting at Track 16 Sector 5
    // follows across Track 16 down to Track 15 Sector 0 and matches test_data byte-for-byte!
    let mut current_track = 16;
    let mut current_sector = 5;
    let mut reconstructed = Vec::new();

    loop {
        let sector_buf = sector_data_map.get(&(current_track, current_sector)).expect("sector missing in chain");
        let next_track = sector_buf[0];
        let next_sector = sector_buf[1];

        if next_track == 0 {
            // Last block: next_sector is the last valid byte offset + 1
            let valid_len = (next_sector as usize).saturating_sub(1);
            reconstructed.extend_from_slice(&sector_buf[2..2 + valid_len]);
            break;
        } else {
            // Full block: 254 bytes
            reconstructed.extend_from_slice(&sector_buf[2..256]);
            current_track = next_track;
            current_sector = next_sector;
        }
    }

    assert_eq!(reconstructed.len(), test_data.len(), "Reconstructed length must match");
    assert_eq!(reconstructed, test_data, "Reconstructed bytes must match test_data byte-for-byte");
}

#[test]
fn test_root_directory_virtual_disk_track_18() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::{DirectoryEntry, VectorDirectorySource, MemoryStorage};
    use pico1541_manager::DiskImage;
    use pico1541_manager::gcr::decode_gcr_block;
    use pollster::block_on;

    let mut entries = heapless::Vec::<DirectoryEntry, 2>::new();
    entries.push(DirectoryEntry::new_dir(b"INTERNAL\0extra", 17, 0)).expect("push failed");
    entries.push(DirectoryEntry::new_dir(b"SD CARD", 17, 1)).expect("push failed");

    let source = VectorDirectorySource::new(b"STORAGE", entries);
    let mut vdisk = VirtualDiskImage::<MemoryStorage<'static>, _>::new(source, b"STORAGE");

    let mut dest = vec![0u8; 10000];
    let (layout, len, _) = block_on(vdisk.read_track(34, &mut dest)).unwrap();
    assert_eq!(layout, pico1541_common::track_buffer::TrackLayout::Accurate);

    // Decode all sectors into map
    let mut sector_data_map = std::collections::HashMap::new();
    let mut pos = 0;
    while pos + 5 <= len {
        if dest[pos..pos + 5] == [0xFF, 0xFF, 0xFF, 0xFF, 0xFF] {
            let mut header_raw = [0u8; 8];
            decode_gcr_block((&dest[pos + 5..pos + 10]).try_into().unwrap(), (&mut header_raw[0..4]).try_into().unwrap());
            decode_gcr_block((&dest[pos + 10..pos + 15]).try_into().unwrap(), (&mut header_raw[4..8]).try_into().unwrap());
            if header_raw[0] == 0x08 {
                let sector = header_raw[2];
                let data_gcr_pos = pos + 5 + 10 + 9 + 5;
                if data_gcr_pos + 325 <= len {
                    let mut data_raw = [0u8; 260];
                    for chunk_idx in 0..65 {
                        let src_slice = &dest[data_gcr_pos + chunk_idx * 5..data_gcr_pos + chunk_idx * 5 + 5];
                        let dst_slice = &mut data_raw[chunk_idx * 4..chunk_idx * 4 + 4];
                        decode_gcr_block(src_slice.try_into().unwrap(), dst_slice.try_into().unwrap());
                    }
                    assert_eq!(data_raw[0], 0x07);
                    let mut sector_buf = [0u8; 256];
                    sector_buf.copy_from_slice(&data_raw[1..257]);
                    sector_data_map.insert(sector, sector_buf);
                    pos = data_gcr_pos + 325;
                    continue;
                }
            }
        }
        pos += 1;
    }

    assert_eq!(sector_data_map.len(), 19, "All 19 sectors must be present on Track 18");

    // Verify Sector 0 (BAM) points to (18, 1) and disk name is STORAGE
    let s0 = sector_data_map.get(&0).unwrap();
    assert_eq!(s0[0], 18);
    assert_eq!(s0[1], 1);
    assert_eq!(&s0[0x90..0x97], b"STORAGE");

    // Verify Sector 1: points to (0, 0xFF), contains INTERNAL and SD CARD
    let s1 = sector_data_map.get(&1).unwrap();
    assert_eq!(s1[0], 0);
    assert_eq!(s1[1], 0xFF);

    // Entry 0: INTERNAL
    assert_eq!(s1[2], 0x86, "First entry must be DIR (0x86)");
    assert_eq!(s1[3], 17, "First entry target track must be 17");
    assert_eq!(s1[4], 0, "First entry target sector must be 0");
    assert_eq!(&s1[5..13], b"INTERNAL");

    // Entry 1: SD CARD
    assert_eq!(s1[0x20 + 2], 0x86, "Second entry must be DIR (0x86)");
    assert_eq!(s1[0x20 + 3], 17, "Second entry target track must be 17");
    assert_eq!(s1[0x20 + 4], 1, "Second entry target sector must be 1");
    assert_eq!(&s1[0x20 + 5..0x20 + 12], b"SD CARD");

    // Entries 2..7 must be empty (file_type == 0)
    for entry_idx in 2..8 {
        assert_eq!(s1[entry_idx * 32 + 2], 0, "Unused entry must be 0");
    }
}

#[test]
fn test_g64_image_with_weak_bits() {
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
    // Fill with 0x55 but insert a weak bit run (0x00)
    for i in 0..track_len as usize {
        data[102 + i] = 0x55;
    }
    data[102 + 10..102 + 15].fill(0x00); // 5 weak bytes

    let storage = MemoryStorage::new(&data);
    let mut img = block_on(G64Image::from_storage(storage)).unwrap();

    let raw_buf = Box::into_raw(Box::new([[0u8; 8192]; 2]));
    let dual_buf = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new(raw_buf)));
    let (manager, emulator) = dual_buf.split();

    let dest = manager.start_track_load();
    let (layout, len, _) = block_on(img.read_track(0, dest)).unwrap();
    manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier(0), layout, len);

    let active_buf = emulator.get_active_track_buffer();
    assert_eq!(active_buf.weak_range_count, 1);
    assert_eq!(
        active_buf.weak_ranges[0],
        Some(pico1541_common::track_buffer::WeakBitRange {
            offset: 10,
            length: 5
        })
    );
    let slice = emulator.get_active_buffer_slice();
    for &b in &slice[10..15] {
        assert_ne!(b, 0x00);
        assert_ne!(b, 0xFF);
    }
}

