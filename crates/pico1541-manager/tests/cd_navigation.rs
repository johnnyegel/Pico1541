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


