use pico1541_manager::tasks::resolve_cd_path;

#[test]
fn test_resolve_cd_enter_subfolder() {
    let current = heapless::String::<128>::try_from("/").unwrap();
    let target = resolve_cd_path(&current, "CD:GAMES").unwrap();
    assert_eq!(target.as_str(), "/GAMES");
}

#[test]
fn test_resolve_cd_enter_nested_subfolder() {
    let current = heapless::String::<128>::try_from("/GAMES").unwrap();
    let target = resolve_cd_path(&current, "CD:ACTION").unwrap();
    assert_eq!(target.as_str(), "/GAMES/ACTION");
}

#[test]
fn test_resolve_cd_parent_left_arrow() {
    let current = heapless::String::<128>::try_from("/GAMES/ACTION").unwrap();
    let target = resolve_cd_path(&current, "CD:\x1F").unwrap();
    assert_eq!(target.as_str(), "/GAMES");
}

#[test]
fn test_resolve_cd_parent_dotdot() {
    let current = heapless::String::<128>::try_from("/GAMES/ACTION").unwrap();
    let target = resolve_cd_path(&current, "CD:..").unwrap();
    assert_eq!(target.as_str(), "/GAMES");
}

#[test]
fn test_resolve_cd_parent_underscore() {
    let current = heapless::String::<128>::try_from("/GAMES").unwrap();
    let target = resolve_cd_path(&current, "CD:_").unwrap();
    assert_eq!(target.as_str(), "/");
}

#[test]
fn test_resolve_cd_root_double_slash() {
    let current = heapless::String::<128>::try_from("/GAMES/ACTION").unwrap();
    let target = resolve_cd_path(&current, "CD//").unwrap();
    assert_eq!(target.as_str(), "/");
}

#[test]
fn test_resolve_cd_parent_from_root_stays_root() {
    let current = heapless::String::<128>::try_from("/").unwrap();
    let target = resolve_cd_path(&current, "CD:\x1F").unwrap();
    assert_eq!(target.as_str(), "/");
}

#[test]
fn test_resolve_cd_alternate_prefix_slash() {
    let current = heapless::String::<128>::try_from("/").unwrap();
    let target = resolve_cd_path(&current, "CD/DEMOS").unwrap();
    assert_eq!(target.as_str(), "/DEMOS");
}

#[test]
fn test_resolve_cd_case_insensitive() {
    let current = heapless::String::<128>::try_from("/").unwrap();
    let target = resolve_cd_path(&current, "cd:GAMES").unwrap();
    assert_eq!(target.as_str(), "/GAMES");
}

#[test]
fn test_resolve_cd_unrecognized_returns_none() {
    let current = heapless::String::<128>::try_from("/").unwrap();
    let result = resolve_cd_path(&current, "LOAD\"$\",8");
    assert!(result.is_none());
}

#[test]
fn test_resolve_cd_disk_image_path() {
    let current = heapless::String::<128>::try_from("/GAMES").unwrap();
    let target = resolve_cd_path(&current, "CD:PITSTOP.D64").unwrap();
    assert_eq!(target.as_str(), "/GAMES/PITSTOP.D64");
}

#[test]
fn test_resolve_cd_action_navigation() {
    use pico1541_manager::tasks::mount_manager::cd_handler::{resolve_cd_action, CdAction};

    let h = |s: &str| heapless::String::<128>::try_from(s).unwrap();

    assert_eq!(resolve_cd_action("/", "CD:>>> MORE >>>"), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:..."), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:<<< PREV <<<"), Some(CdAction::PrevPage));
    assert_eq!(resolve_cd_action("/GAMES", "CD:.."), Some(CdAction::ChangeDir(h("/"))));
    assert_eq!(resolve_cd_action("/", "CD:GAMES"), Some(CdAction::ChangeDir(h("/GAMES"))));
    assert_eq!(resolve_cd_action("/", "CD:GAME.D64"), Some(CdAction::MountImage(h("/GAME.D64"))));
    assert_eq!(resolve_cd_action("/", "CD/GAME.G64"), Some(CdAction::MountImage(h("/GAME.G64"))));
    assert_eq!(resolve_cd_action("/", "CD/GAME.NIB"), Some(CdAction::MountImage(h("/GAME.NIB"))));
}

#[test]
fn test_resolve_cd_action_absolute_path() {
    use pico1541_manager::tasks::mount_manager::cd_handler::{resolve_cd_action, CdAction};

    let h = |s: &str| heapless::String::<128>::try_from(s).unwrap();

    // Absolute paths from a subdirectory
    assert_eq!(
        resolve_cd_action("/SUB/NESTED", "CD:/GAMES"),
        Some(CdAction::ChangeDir(h("/GAMES")))
    );
    assert_eq!(
        resolve_cd_action("/SUB/NESTED", "CD//GAMES"),
        Some(CdAction::ChangeDir(h("/GAMES")))
    );
    assert_eq!(
        resolve_cd_action("/SUB", "CD:/IMAGE.D64"),
        Some(CdAction::MountImage(h("/IMAGE.D64")))
    );
}

#[test]
fn test_resolve_cd_non_ascii_safety() {
    use pico1541_manager::tasks::mount_manager::cd_handler::resolve_cd_action;

    // Non-ASCII characters should not panic in strip_prefix_ci
    assert_eq!(resolve_cd_action("/", "§§§"), None);
    assert_eq!(resolve_cd_action("/", "äöü"), None);
    assert_eq!(resolve_cd_action("/", "日本語"), None);
    assert_eq!(resolve_cd_action("/", "€"), None);
}

#[test]
fn test_cd_action_derive_eq() {
    use pico1541_manager::tasks::mount_manager::cd_handler::CdAction;

    fn assert_eq_impl<T: Eq>() {}
    assert_eq_impl::<CdAction>();
}

#[test]
fn test_paged_directory_page_1_few_entries() {
    use pico1541_manager::storage::{
        DirectoryEntry, DirectorySource, PagedDirectorySource, PagedDirectorySourceTrait,
        PAGE_CAPACITY,
    };

    let mut source = PagedDirectorySource::<144>::new(b"TEST DIR");

    // Populate Page 1 with 5 entries using set_page_entries (count < PAGE_CAPACITY, has_more: false)
    let page1_entries: Vec<DirectoryEntry> = (0..5)
        .map(|i| {
            let name = format!("FILE_P1_{}", i);
            DirectoryEntry::new(name.as_bytes(), 17, 0, 10)
        })
        .collect();

    source.set_page_entries(1, &page1_entries, false);

    assert_eq!(source.current_page(), 1);
    assert_eq!(source.total_count(), PAGE_CAPACITY + 5);

    // Reading should yield:
    // 1) PREV navigation entry
    // 2..6) 5 real entries
    // 7) None (end of page)
    let first = source.next_entry().expect("Should have PREV entry");
    assert_eq!(&first.filename[..12], b"<<< PREV <<<");

    for i in 0..5 {
        let entry = source.next_entry().expect("Should have real entry");
        let expected = format!("FILE_P1_{}", i);
        assert_eq!(&entry.filename[..expected.len()], expected.as_bytes());
    }

    assert!(source.next_entry().is_none(), "Should terminate after 5 entries");
}


