use pico1541_common::location::DriveLocation;
use heapless::String;

#[test]
fn test_drive_location_variants_and_helpers() {
    let root = DriveLocation::Root;
    assert!(root.is_root());
    assert_eq!(root.path(), "/");
    assert!(!root.use_internal());

    let vdir = DriveLocation::VirtualDirectory {
        path: String::try_from("/files").unwrap(),
        use_internal: true,
    };
    assert!(!vdir.is_root());
    assert_eq!(vdir.path(), "/files");
    assert!(vdir.use_internal());

    let img = DriveLocation::DiskImage {
        path: String::try_from("/GAMES/TEST.D64").unwrap(),
        use_internal: false,
    };
    assert!(!img.is_root());
    assert_eq!(img.path(), "/GAMES/TEST.D64");
    assert!(!img.use_internal());
}
