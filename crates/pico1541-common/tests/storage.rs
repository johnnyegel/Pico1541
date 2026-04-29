use pico1541_common::storage::{MemoryStorage, DirectoryEntry};
use embedded_io_async::{Read, Seek, SeekFrom};
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
