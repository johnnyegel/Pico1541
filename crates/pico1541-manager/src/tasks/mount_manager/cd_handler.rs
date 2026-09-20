/// Resolves an SD2IEC `CD:` command string against the current directory path.
///
/// # Arguments
/// - `current_path`: The current directory path (e.g. `"/"`, `"/GAMES"`, `"/GAMES/ACTION"`).
/// - `cd_cmd`: The raw CD command string as sent from the C64 (e.g. `"CD:GAMES"`, `"CD/"`, `"CD:\x1F"`).
///
/// # Returns
/// `Some(new_path)` on success, or `None` if the command is unrecognized.
///
/// # Supported commands
/// - `CD:dirname`  — enter named subfolder
/// - `CD:\x1F`    — go to parent directory (PETSCII left-arrow)
/// - `CD:_`       — go to parent directory (alternate encoding used by some tools)
/// - `CD:..`      — go to parent directory
/// - `CD//`       — go to root
/// - `CD/dirname` — enter named subfolder (alternate prefix)
pub fn resolve_cd_path(
    current_path: &str,
    cd_cmd: &str,
) -> Option<heapless::String<128>> {
    // Strip prefix to obtain the raw argument
    let raw_arg = if cd_cmd.eq_ignore_ascii_case("CD//") || cd_cmd.eq_ignore_ascii_case("CD:") {
        // CD// or bare CD: → root
        return heapless::String::try_from("/").ok();
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
        return heapless::String::try_from("/").ok();
    }

    // Parent directory: PETSCII left-arrow (0x1F), "..", or "_"
    if arg == "\x1F" || arg == ".." || arg == "_" {
        return go_to_parent(current_path);
    }

    // Child folder / image file
    go_to_child(current_path, arg)
}

// ── helpers ────────────────────────────────────────────────────────────────

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() {
        let (head, tail) = s.split_at(prefix.len());
        if head.eq_ignore_ascii_case(prefix) {
            return Some(tail);
        }
    }
    None
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
