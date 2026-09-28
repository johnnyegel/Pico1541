#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CdAction {
    NextPage,
    PrevPage,
    ChangeDir(heapless::String<128>),
    MountImage(heapless::String<128>),
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


// ── helpers ────────────────────────────────────────────────────────────────

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn go_to_parent(current_path: &str) -> Option<heapless::String<128>> {
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

fn go_to_child(current_path: &str, name: &str) -> Option<heapless::String<128>> {
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
