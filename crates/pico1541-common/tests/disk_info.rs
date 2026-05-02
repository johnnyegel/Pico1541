use pico1541_common::disk_info::{inspect_d64, petscii_to_ascii, D64_TRACK18_OFFSET};

#[test]
fn test_petscii_to_ascii() {
    let petscii = b"HELLO\xA0WORLD\xA0\xA0\xA0";
    let mut ascii = [0u8; 16];
    let len = petscii_to_ascii(petscii, &mut ascii);
    assert_eq!(&ascii[..len], b"HELLO WORLD");
    assert_eq!(len, 11);

    let petscii = [0xA0u8; 16];
    let len = petscii_to_ascii(&petscii, &mut ascii);
    assert_eq!(len, 0);
}

#[test]
fn test_inspect_d64() {
    let mut data = vec![0u8; 174848];
    let bam_offset = D64_TRACK18_OFFSET;

    // Disk Name: "TEST DISK" (padded with 0xA0)
    let name = b"TEST DISK";
    data[bam_offset + 144..bam_offset + 144 + name.len()].copy_from_slice(name);
    for i in name.len()..16 {
        data[bam_offset + 144 + i] = 0xA0;
    }

    // Disk ID: "XY"
    data[bam_offset + 162] = b'X';
    data[bam_offset + 163] = b'Y';

    let info = inspect_d64(&data).unwrap();
    assert!(info.valid_size);
    assert_eq!(info.disk_id, [b'X', b'Y']);
    assert_eq!(&info.disk_name_petscii[..9], b"TEST DISK");
    assert_eq!(info.disk_name_petscii[9], 0xA0);

    let mut ascii_name = [0u8; 16];
    let len = petscii_to_ascii(&info.disk_name_petscii, &mut ascii_name);
    assert_eq!(&ascii_name[..len], b"TEST DISK");
}

#[test]
fn test_inspect_d64_invalid_size() {
    let data = vec![0u8; 1000];
    assert!(inspect_d64(&data).is_none());

    let data = vec![0u8; 200000];
    let info = inspect_d64(&data).unwrap();
    assert!(!info.valid_size);
}
