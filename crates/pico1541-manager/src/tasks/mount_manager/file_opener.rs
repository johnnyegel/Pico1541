#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildInfo {
    pub name: heapless::String<64>,
    pub is_dir: bool,
}

pub trait FileOpener {
    type FileStorage: embedded_io_async::Read + embedded_io_async::Seek + 'static;
    async fn open_file(
        &self,
        requested_petscii: &[u8],
    ) -> Result<Self::FileStorage, crate::formats::Error>;
    fn open_sd_file(&self, path: &str) -> Self::FileStorage;
    fn open_internal_file(&self, path: &str) -> Self::FileStorage;
    async fn list_dir_range(
        &self,
        path: &str,
        use_internal: bool,
        start_index: usize,
        count: usize,
        callback: impl FnMut(crate::storage::DirectoryEntry, &str),
    ) -> Result<(), crate::formats::Error>;
}

pub fn match_c64_filename(entry_name: &[u8; 16], req_name: &[u8]) -> bool {
    let mut entry_len = 16;
    while entry_len > 0 && entry_name[entry_len - 1] == 0xA0 {
        entry_len -= 1;
    }

    let mut req_len = req_name.len();
    while req_len > 0 && req_name[req_len - 1] == 0xA0 {
        req_len -= 1;
    }

    if req_len == 0 || entry_len == 0 {
        return false;
    }

    let mut e_idx = 0;
    let mut r_idx = 0;

    while r_idx < req_len && e_idx < entry_len {
        let r_char = req_name[r_idx];
        if r_char == 0x2A {
            // '*'
            return true;
        } else if r_char == 0x3F { // '?'
             // matches any char
        } else if entry_name[e_idx] != r_char {
            break;
        }
        e_idx += 1;
        r_idx += 1;
    }

    if r_idx < req_len && req_name[r_idx] == 0x2A {
        return true;
    }

    if r_idx == req_len && e_idx == entry_len {
        return true;
    }

    if r_idx == req_len {
        let extensions: &[&[u8]] = &[
            b".PRG", b".SEQ", b".USR", b".DEL", b".DIR", b".D64", b".G64",
        ];
        for ext in extensions {
            if entry_len == r_idx + ext.len() {
                let mut match_found = true;
                for i in 0..ext.len() {
                    let ext_char = crate::storage::ascii_to_petscii(ext[i]);
                    if entry_name[r_idx + i] != ext_char {
                        match_found = false;
                        break;
                    }
                }
                if match_found {
                    return true;
                }
            }
        }
    }

    false
}
