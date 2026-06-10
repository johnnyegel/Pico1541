use pico1541_common::formats::{D64Image, DiskImage, G64Image, VirtualDiskImage};
use pico1541_common::track_buffer::TrackLayout;
use pico1541_common::storage::{DirectoryEntry, DirectorySource, MemoryStorage};
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
    let mut img = D64Image::new(storage);
    let mut dest = vec![0u8; 8000];

    // Track 1.0 is half_track 2
    let result = block_on(img.read_track(2, &mut dest)).unwrap();
    assert!(matches!(result, (TrackLayout::Accurate, 7692)));

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
    let mut img = G64Image::new(storage);
    let mut dest = vec![0u8; 100];

    let result = block_on(img.read_track(0, &mut dest)).unwrap();
    assert!(matches!(result, (TrackLayout::Accurate, 50)));
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

    let result = block_on(img.read_track(34, &mut dest)).unwrap();
    assert!(matches!(result, (TrackLayout::Accurate, 7142)));
    assert!(dest.iter().any(|&b| b != 0));
}
