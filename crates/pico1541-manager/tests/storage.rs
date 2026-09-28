use embedded_io_async::{Read, Seek, SeekFrom};
use pico1541_manager::storage::{DirectoryEntry, MemoryStorage};
use pollster::block_on;

#[test]
fn test_memory_storage_read() {
    let data = b"Hello, World!";
    let mut storage = MemoryStorage::new(data);
    let mut buf = [0u8; 5];

    let n = block_on(storage.read(&mut buf)).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf, b"Hello");

    let n = block_on(storage.read(&mut buf)).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf, b", Wor");

    let mut buf2 = [0u8; 10];
    let n = block_on(storage.read(&mut buf2)).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf2[..n], b"ld!");
}

#[test]
fn test_memory_storage_seek() {
    let data = b"0123456789";
    let mut storage = MemoryStorage::new(data);

    block_on(storage.seek(SeekFrom::Start(5))).unwrap();
    let mut buf = [0u8; 1];
    block_on(storage.read(&mut buf)).unwrap();
    assert_eq!(buf[0], b'5');

    block_on(storage.seek(SeekFrom::Current(2))).unwrap();
    block_on(storage.read(&mut buf)).unwrap();
    assert_eq!(buf[0], b'8');

    block_on(storage.seek(SeekFrom::End(-2))).unwrap();
    block_on(storage.read(&mut buf)).unwrap();
    assert_eq!(buf[0], b'8');
}

#[test]
fn test_directory_entry() {
    let filename = b"MY FILE";
    let entry = DirectoryEntry::new(filename, 18, 1, 10);
    assert_eq!(entry.file_type, 0x82);
    assert_eq!(entry.first_track, 18);
    assert_eq!(entry.first_sector, 1);
    assert_eq!(entry.block_size, 10);
    assert_eq!(&entry.filename[..7], b"MY FILE");
    assert_eq!(entry.filename[7], 0xA0);
}

#[test]
fn test_paged_directory_source() {
    use pico1541_manager::storage::{
        DirectorySource, PagedDirectorySource, PagedDirectorySourceTrait,
    };

    let mut paged = PagedDirectorySource::<48>::new(b"TEST DRIVE");
    // Before any block is loaded total_count is usize::MAX (unknown)
    assert_eq!(paged.total_count(), usize::MAX);
    assert!(!paged.is_exhausted());
    assert_eq!(paged.loaded_range(), (0, 0));

    // Populate block 0
    let mut block0_entries = [DirectoryEntry::default(); 16];
    for (i, entry) in block0_entries.iter_mut().enumerate() {
        *entry = DirectoryEntry::new(format!("FILE{:02}", i).as_bytes(), 18, i as u8, 1);
    }
    paged.set_block(0, &block0_entries);
    assert_eq!(paged.loaded_range(), (0, 1));

    // Populate block 1
    let mut block1_entries = [DirectoryEntry::default(); 16];
    for (i, entry) in block1_entries.iter_mut().enumerate() {
        *entry = DirectoryEntry::new(format!("FILE{:02}", 16 + i).as_bytes(), 18, i as u8, 1);
    }
    paged.set_block(1, &block1_entries);
    assert_eq!(paged.loaded_range(), (0, 2));

    // Populate block 2
    let mut block2_entries = [DirectoryEntry::default(); 16];
    for (i, entry) in block2_entries.iter_mut().enumerate() {
        *entry = DirectoryEntry::new(format!("FILE{:02}", 32 + i).as_bytes(), 18, i as u8, 1);
    }
    paged.set_block(2, &block2_entries);
    assert_eq!(paged.loaded_range(), (0, 3));

    // Read entries from block 0, 1, 2
    paged.rewind();
    for i in 0..48 {
        let entry = paged.next_entry().expect("Should have entry");
        let name_str = format!("FILE{:02}", i);
        assert_eq!(&entry.filename[..name_str.len()], name_str.as_bytes());
    }

    // Since block 3 isn't loaded, reading index 48 should return None
    assert!(paged.next_entry().is_none());

    // Populate block 3 (causes sliding window)
    let mut block3_entries = [DirectoryEntry::default(); 16];
    for (i, entry) in block3_entries.iter_mut().enumerate() {
        *entry = DirectoryEntry::new(format!("FILE{:02}", 48 + i).as_bytes(), 18, i as u8, 1);
    }
    paged.set_block(3, &block3_entries);
    // Range slides to (1, 4)
    assert_eq!(paged.loaded_range(), (1, 4));

    // Rewind and read. Indices 0..16 are now missing because block 0 was evicted (slide range is 1..4)
    paged.rewind();
    assert!(paged.next_entry().is_none()); // index 0 is not in range 1..4!
}

#[test]
fn test_paged_directory_entry_at_slot() {
    use pico1541_manager::storage::{DirectoryEntry, PagedDirectorySource};

    let mut paged = PagedDirectorySource::<144>::new(b"TEST DISK");
    let entry0 = DirectoryEntry::from_file_info(b"GAME1.PRG", false, 500);
    let entry1 = DirectoryEntry::from_file_info(b"GAME2.PRG", false, 1000);
    paged.set_page_entry(0, entry0);
    paged.set_page_entry(1, entry1);
    paged.set_total_count(2);

    // Slot 0 -> track 17, sector 0
    let s0 = paged.entry_at_slot(0).expect("slot 0 must exist");
    assert_eq!(s0.first_track, 17);
    assert_eq!(s0.first_sector, 0);
    assert!(!s0.is_dir());

    // Slot 1 -> track 17, sector 1
    let s1 = paged.entry_at_slot(1).expect("slot 1 must exist");
    assert_eq!(s1.first_track, 17);
    assert_eq!(s1.first_sector, 1);
    assert!(!s1.is_dir());

    // Slot 2 beyond total_count -> None
    assert!(paged.entry_at_slot(2).is_none());

    // Test on Page 1 (where slot 0 is <<< PREV <<<)
    paged.set_page(1);
    paged.set_total_count(200); // more entries than page capacity
    let prev = paged.entry_at_slot(0).expect("slot 0 should be PREV");
    assert!(prev.is_dir());
    assert_eq!(&prev.filename[..12], b"<<< PREV <<<");
}

