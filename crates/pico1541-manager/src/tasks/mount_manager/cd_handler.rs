use heapless::String;
use pico1541_common::DriveLocation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdAction {
    NextPage,
    PrevPage,
    ChangeDir(heapless::String<128>),
    MountImage(heapless::String<128>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CdNavAction {
    NextPage,
    PrevPage,
    Transition(DriveLocation),
    Ignore,
}

pub fn resolve_cd_nav(current: &DriveLocation, cmd: &str) -> CdNavAction {
    let cmd = cmd.trim();
    let body = if cmd.eq_ignore_ascii_case("CD//") || cmd.eq_ignore_ascii_case("CD:") {
        ""
    } else if let Some(stripped) = strip_prefix_ci(cmd, "CD:") {
        stripped
    } else if let Some(stripped) = strip_prefix_ci(cmd, "CD/") {
        stripped
    } else {
        return CdNavAction::Ignore;
    };

    let body = body.trim();

    // Page navigation commands
    if body.eq_ignore_ascii_case(">>> MORE >>>") || body == "..." {
        return CdNavAction::NextPage;
    }
    if body.eq_ignore_ascii_case("<<< PREV <<<") {
        return CdNavAction::PrevPage;
    }

    match current {
        DriveLocation::Root => {
            let clean_body = body.trim_matches('/');
            if clean_body.eq_ignore_ascii_case("INTERNAL") {
                CdNavAction::Transition(DriveLocation::VirtualDirectory {
                    path: String::try_from("/files").unwrap_or_default(),
                    use_internal: true,
                })
            } else if clean_body.eq_ignore_ascii_case("SD CARD")
                || clean_body.eq_ignore_ascii_case("SD")
            {
                CdNavAction::Transition(DriveLocation::VirtualDirectory {
                    path: String::try_from("/").unwrap_or_default(),
                    use_internal: false,
                })
            } else {
                CdNavAction::Ignore
            }
        }
        DriveLocation::VirtualDirectory { path, use_internal } => {
            // Returning to Root or parent
            if body.is_empty() || body == "/" || body == "//" {
                return CdNavAction::Transition(DriveLocation::Root);
            }
            if body == ".." || body == "_" || body == "\x1F" || body == "<-" {
                if (*use_internal && path == "/files") || (!*use_internal && path == "/") {
                    return CdNavAction::Transition(DriveLocation::Root);
                }
                let trimmed = path.trim_end_matches('/');
                let parent = match trimmed.rfind('/') {
                    Some(0) => "/",
                    Some(idx) => &trimmed[..idx],
                    None => "/",
                };
                return CdNavAction::Transition(DriveLocation::VirtualDirectory {
                    path: String::try_from(parent).unwrap_or_default(),
                    use_internal: *use_internal,
                });
            }

            // Check if mounting a disk image (.d64, .g64, .nib)
            let is_image = is_disk_image(body);

            let mut target_path = String::<128>::new();
            let append_ok = if body.starts_with('/') {
                target_path.push_str(body).is_ok()
            } else {
                let base = path.trim_end_matches('/');
                let mut ok = true;
                if !base.is_empty() {
                    ok = target_path.push_str(base).is_ok();
                }
                if ok {
                    ok = target_path.push('/').is_ok();
                }
                if ok {
                    ok = target_path.push_str(body).is_ok();
                }
                ok
            };

            if !append_ok {
                return CdNavAction::Ignore;
            }

            if is_image {
                CdNavAction::Transition(DriveLocation::DiskImage {
                    path: target_path,
                    use_internal: *use_internal,
                })
            } else {
                CdNavAction::Transition(DriveLocation::VirtualDirectory {
                    path: target_path,
                    use_internal: *use_internal,
                })
            }
        }
        DriveLocation::DiskImage { path, use_internal } => {
            if body == ".." || body == "_" || body.is_empty() || body == "/" || body == "//" || body == "\x1F" || body == "<-" {
                let trimmed = path.trim_end_matches('/');
                let parent = match trimmed.rfind('/') {
                    Some(0) => "/",
                    Some(idx) => &trimmed[..idx],
                    None => "/",
                };
                CdNavAction::Transition(DriveLocation::VirtualDirectory {
                    path: String::try_from(parent).unwrap_or_default(),
                    use_internal: *use_internal,
                })
            } else {
                CdNavAction::Ignore
            }
        }
    }
}


/// Resolves an SD2IEC `CD:` command string into a [`CdAction`].
///
/// # Arguments
/// - `current_path`: The current directory path (e.g. `"/"`, `"/GAMES"`).
/// - `cd_cmd`: The raw CD command string as sent from the C64.
///
/// # Returns
/// `Some(CdAction)` on success, or `None` if the command is unrecognized.
pub fn resolve_cd_action(
    current_path: &str,
    cd_cmd: &str,
) -> Option<CdAction> {
    // Strip prefix to obtain the raw argument
    let raw_arg = if cd_cmd.eq_ignore_ascii_case("CD//") || cd_cmd.eq_ignore_ascii_case("CD:") {
        // CD// or bare CD: → root
        return Some(CdAction::ChangeDir(heapless::String::try_from("/").ok()?));
    } else if let Some(stripped) = strip_prefix_ci(cd_cmd, "CD:") {
        stripped
    } else if let Some(stripped) = strip_prefix_ci(cd_cmd, "CD/") {
        stripped
    } else {
        return None;
    };

    // Trim trailing spaces that the C64 may pad
    let arg = raw_arg.trim_end();

    // Empty argument → root
    if arg.is_empty() || arg == "/" || arg == "//" {
        return Some(CdAction::ChangeDir(heapless::String::try_from("/").ok()?));
    }

    // Page navigation commands
    if arg == ">>> MORE >>>" || arg == "..." {
        return Some(CdAction::NextPage);
    }
    if arg == "<<< PREV <<<" {
        return Some(CdAction::PrevPage);
    }

    // Parent directory: PETSCII left-arrow (0x1F), "..", or "_"
    let new_path = if arg == "\x1F" || arg == ".." || arg == "_" {
        go_to_parent(current_path)?
    } else if arg.starts_with('/') {
        heapless::String::try_from(arg).ok()?
    } else {
        go_to_child(current_path, arg)?
    };

    let p_str = new_path.as_str();
    let is_image = if let Some(dot) = p_str.rfind('.') {
        let ext = &p_str[dot..];
        ext.eq_ignore_ascii_case(".d64")
            || ext.eq_ignore_ascii_case(".g64")
            || ext.eq_ignore_ascii_case(".nib")
    } else {
        false
    };

    if is_image {
        Some(CdAction::MountImage(new_path))
    } else {
        Some(CdAction::ChangeDir(new_path))
    }
}

/// Resolves an SD2IEC `CD:` command string against the current directory path.
///
/// Backwards-compatible wrapper around [`resolve_cd_action`].
pub fn resolve_cd_path(
    current_path: &str,
    cd_cmd: &str,
) -> Option<heapless::String<128>> {
    match resolve_cd_action(current_path, cd_cmd)? {
        CdAction::ChangeDir(path) | CdAction::MountImage(path) => Some(path),
        CdAction::NextPage | CdAction::PrevPage => None,
    }
}


pub fn extract_cd_target(cmd: &str) -> &str {
    let trimmed = cmd.trim();
    let body = if let Some(stripped) = strip_prefix_ci(trimmed, "CD:") {
        stripped.trim()
    } else if let Some(stripped) = strip_prefix_ci(trimmed, "CD/") {
        stripped.trim()
    } else {
        trimmed
    };
    body.trim_matches('/')
}

pub fn is_disk_image(path: &str) -> bool {
    if let Some(dot) = path.rfind('.') {
        let ext = &path[dot..];
        ext.eq_ignore_ascii_case(".d64")
            || ext.eq_ignore_ascii_case(".g64")
            || ext.eq_ignore_ascii_case(".nib")
    } else {
        false
    }
}

pub(crate) fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

pub fn go_to_parent(current_path: &str) -> Option<heapless::String<128>> {
    if current_path == "/" || current_path.is_empty() {
        return heapless::String::try_from("/").ok();
    }
    let trimmed = current_path.trim_end_matches('/');
    let parent = if let Some(idx) = trimmed.rfind('/') {
        if idx == 0 {
            "/"
        } else {
            &trimmed[..idx]
        }
    } else {
        "/"
    };
    heapless::String::try_from(parent).ok()
}

pub fn go_to_child(current_path: &str, name: &str) -> Option<heapless::String<128>> {
    let mut new_path = heapless::String::<128>::new();
    // Append base path without trailing slash
    let base = current_path.trim_end_matches('/');
    if !base.is_empty() {
        new_path.push_str(base).ok()?;
    }
    new_path.push('/').ok()?;
    new_path.push_str(name).ok()?;
    Some(new_path)
}
