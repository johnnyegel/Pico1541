use core::cmp;
use embedded_io_async::{ErrorType, Read, Seek, SeekFrom};

pub struct MemoryStorage<'a> {
    data: &'a [u8],
    pos: usize,
}

pub fn ascii_to_petscii(a: u8) -> u8 {
    match a {
        b'a'..=b'z' => a - 0x20, // 0x61-0x7A -> 0x41-0x5A (lowercase to uppercase PETSCII)
        // A-Z (0x41-0x5A) are already identical in ASCII and unshifted PETSCII
        _ => a,
    }
}

pub fn petscii_to_ascii(p: u8) -> u8 {
    match p {
        0x41..=0x5A => p + 0x20, // 0x41-0x5A -> 0x61-0x7A ('a'-'z')
        0xC1..=0xDA => p - 0x80, // 0xC1-0xDA -> 0x41-0x5A ('A'-'Z')
        _ => p,
    }
}

pub fn mangle_filename(name: &[u8]) -> [u8; 16] {
    let mut petscii = [0xA0; 16];
    let actual_len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let actual_name = &name[..actual_len];

    if actual_name.len() <= 16 {
        for (i, &b) in actual_name.iter().enumerate() {
            petscii[i] = ascii_to_petscii(b);
        }
    } else {
        // Take first 13 chars
        for i in 0..13 {
            petscii[i] = ascii_to_petscii(actual_name[i]);
        }
        petscii[13] = b'-'; // 0x2D is universally supported in PETSCII

        if actual_len >= 2 {
            petscii[14] = ascii_to_petscii(actual_name[actual_len - 2]);
            petscii[15] = ascii_to_petscii(actual_name[actual_len - 1]);
        }
    }
    petscii
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DirectoryEntry {
    pub file_type: u8,
    pub filename: [u8; 16],
    pub first_track: u8,
    pub first_sector: u8,
    pub block_size: u16,
}

impl Default for DirectoryEntry {
    fn default() -> Self {
        Self {
            file_type: 0x00, // Empty entry
            filename: [0xA0; 16],
            first_track: 0,
            first_sector: 0,
            block_size: 0,
        }
    }
}

impl DirectoryEntry {
    pub fn new(filename: &[u8], first_track: u8, first_sector: u8, block_size: u16) -> Self {
        let mut entry = Self {
            file_type: 0x82, // PRG
            filename: [0xA0; 16],
            first_track,
            first_sector,
            block_size,
        };
        let len = filename.len().min(16);
        entry.filename[..len].copy_from_slice(&filename[..len]);
        entry
    }

    pub fn from_file_info(name: &[u8], is_dir: bool, size: u32) -> Self {
        let filename = mangle_filename(name);

        let file_type = if is_dir {
            0x83 // USR for directories
        } else {
            0x82 // PRG
        };

        Self {
            file_type,
            filename,
            first_track: 1,
            first_sector: 1,
            block_size: if is_dir { 0 } else { size.div_ceil(254) as u16 },
        }
    }
}

pub trait PagedDirectorySourceTrait {
    fn current_read_index(&self) -> usize;
    /// Returns the known upper bound on the number of entries.
    /// Starts at `usize::MAX` and is tightened lazily when the end of directory is discovered.
    fn total_count(&self) -> usize;
    fn loaded_range(&self) -> (usize, usize);
    fn set_block(&mut self, block_index: usize, entries: &[DirectoryEntry]);
    fn set_total_count(&mut self, total_count: usize);
    /// Returns true if the directory has been fully scanned (end found).
    fn is_exhausted(&self) -> bool;
    /// Invalidates the cache and resets the read pointer.
    fn invalidate(&mut self);
}

pub trait DirectorySource {
    fn name(&self) -> &[u8; 16];
    fn next_entry(&mut self) -> Option<DirectoryEntry>;
    fn rewind(&mut self);
    fn find_entry(&mut self, filename: &[u8]) -> Option<DirectoryEntry> {
        self.rewind();
        while let Some(entry) = self.next_entry() {
            let mut match_found = true;
            for (&e_byte, &f_byte) in entry.filename.iter().zip(filename.iter()).take(16) {
                if e_byte != f_byte {
                    match_found = false;
                    break;
                }
            }
            if match_found {
                for &e_byte in entry
                    .filename
                    .iter()
                    .skip(filename.len())
                    .take(16 - filename.len())
                {
                    if e_byte != 0xA0 {
                        match_found = false;
                        break;
                    }
                }
            }
            if match_found {
                return Some(entry);
            }
        }
        None
    }
    fn as_paged_mut(&mut self) -> Option<&mut dyn PagedDirectorySourceTrait> {
        None
    }
}

pub struct DummySource;
impl DirectorySource for DummySource {
    fn name(&self) -> &[u8; 16] {
        &[0xA0; 16]
    }
    fn next_entry(&mut self) -> Option<DirectoryEntry> {
        None
    }
    fn rewind(&mut self) {}
}

pub struct StaticDirectorySource<'a> {
    name: [u8; 16],
    entries: &'a [DirectoryEntry],
    index: usize,
}

impl<'a> StaticDirectorySource<'a> {
    pub fn new(name: &[u8], entries: &'a [DirectoryEntry]) -> Self {
        let mut n = [0xA0; 16];
        let len = name.len().min(16);
        n[..len].copy_from_slice(&name[..len]);
        Self {
            name: n,
            entries,
            index: 0,
        }
    }
}

impl<'a> DirectorySource for StaticDirectorySource<'a> {
    fn name(&self) -> &[u8; 16] {
        &self.name
    }
    fn next_entry(&mut self) -> Option<DirectoryEntry> {
        if self.index < self.entries.len() {
            let entry = self.entries[self.index];
            self.index += 1;
            Some(entry)
        } else {
            None
        }
    }
    fn rewind(&mut self) {
        self.index = 0;
    }
}

pub struct VectorDirectorySource<const N: usize> {
    name: [u8; 16],
    entries: heapless::Vec<DirectoryEntry, N>,
    index: usize,
}

impl<const N: usize> VectorDirectorySource<N> {
    pub fn new(name: &[u8], entries: heapless::Vec<DirectoryEntry, N>) -> Self {
        let mut n = [0xA0; 16];
        let len = name.len().min(16);
        n[..len].copy_from_slice(&name[..len]);
        Self {
            name: n,
            entries,
            index: 0,
        }
    }
}

impl<const N: usize> DirectorySource for VectorDirectorySource<N> {
    fn name(&self) -> &[u8; 16] {
        &self.name
    }
    fn next_entry(&mut self) -> Option<DirectoryEntry> {
        if self.index < self.entries.len() {
            let entry = self.entries[self.index];
            self.index += 1;
            Some(entry)
        } else {
            None
        }
    }
    fn rewind(&mut self) {
        self.index = 0;
    }
}

pub struct PagedDirectorySource<const N: usize = 144> {
    name: [u8; 16],
    entries: [DirectoryEntry; N],
    start_block: usize,
    end_block: usize,
    current_read_index: usize,
    /// Known upper bound; starts at usize::MAX and is tightened when a partial block arrives.
    total_count: usize,
}

impl<const N: usize> PagedDirectorySource<N> {
    /// Create a new paged directory source. Total entry count is discovered lazily.
    pub fn new(name: &[u8]) -> Self {
        let n = mangle_filename(name);
        Self {
            name: n,
            entries: [DirectoryEntry::default(); N],
            start_block: 0,
            end_block: 0,
            current_read_index: 0,
            total_count: usize::MAX,
        }
    }
}

impl<const N: usize> PagedDirectorySourceTrait for PagedDirectorySource<N> {
    fn current_read_index(&self) -> usize {
        self.current_read_index
    }

    fn total_count(&self) -> usize {
        self.total_count
    }

    fn loaded_range(&self) -> (usize, usize) {
        (self.start_block, self.end_block)
    }

    fn is_exhausted(&self) -> bool {
        self.total_count != usize::MAX
    }

    fn invalidate(&mut self) {
        self.current_read_index = 0;
        self.total_count = usize::MAX;
        self.start_block = 0;
        self.end_block = 0;
    }

    fn set_block(&mut self, block_index: usize, block_entries: &[DirectoryEntry]) {
        let slot = block_index % (N / 16);
        let start_slot_idx = slot * 16;
        for i in 0..16 {
            if i < block_entries.len() {
                self.entries[start_slot_idx + i] = block_entries[i];
            } else {
                self.entries[start_slot_idx + i] = DirectoryEntry::default();
            }
        }

        // A partial block (fewer than 16 entries) signals the end of directory.
        // Tighten total_count so prefetching stops and next_entry returns None correctly.
        if block_entries.len() < 16 {
            let discovered_total = block_index * 16 + block_entries.len();
            if discovered_total < self.total_count {
                self.total_count = discovered_total;
            }
        }

        if self.start_block == self.end_block {
            self.start_block = block_index;
            self.end_block = block_index + 1;
        } else if block_index == self.end_block {
            self.end_block += 1;
            if self.end_block - self.start_block > (N / 16) {
                self.start_block = self.end_block - (N / 16);
            }
        } else if block_index + 1 == self.start_block {
            self.start_block = block_index;
            if self.end_block - self.start_block > (N / 16) {
                self.end_block = self.start_block + (N / 16);
            }
        } else if block_index >= self.start_block && block_index < self.end_block {
            // Already in range, do nothing
        } else {
            self.start_block = block_index;
            self.end_block = block_index + 1;
        }
    }

    fn set_total_count(&mut self, total_count: usize) {
        self.total_count = total_count;
    }
}

impl<const N: usize> DirectorySource for PagedDirectorySource<N> {
    fn name(&self) -> &[u8; 16] {
        &self.name
    }

    fn next_entry(&mut self) -> Option<DirectoryEntry> {
        let idx = self.current_read_index;
        // total_count is usize::MAX until end-of-directory is discovered.
        if idx >= self.total_count {
            return None;
        }
        let b = idx / 16;
        if b >= self.start_block && b < self.end_block {
            let entry = self.entries[(b % (N / 16)) * 16 + (idx % 16)];
            self.current_read_index += 1;
            Some(entry)
        } else {
            None // Block not yet loaded; caller must prefetch
        }
    }

    fn rewind(&mut self) {
        self.current_read_index = 0;
    }

    fn as_paged_mut(&mut self) -> Option<&mut dyn PagedDirectorySourceTrait> {
        Some(self)
    }
}

pub trait FileStorage: Read + Seek {}
impl<T: Read + Seek> FileStorage for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Io,
    NotReady,
}

impl embedded_io_async::Error for Error {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        embedded_io_async::ErrorKind::Other
    }
}

impl From<core::convert::Infallible> for Error {
    fn from(_: core::convert::Infallible) -> Self {
        Error::Io
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl core::error::Error for Error {}

pub enum RuntimeStorage<'a, S: FileStorage, S2: FileStorage = S> {
    Memory(MemoryStorage<'a>),
    File(S),
    File2(S2),
    None,
}

impl<'a, S: FileStorage, S2: FileStorage> ErrorType for RuntimeStorage<'a, S, S2> {
    type Error = Error;
}

impl<'a, S: FileStorage, S2: FileStorage> Read for RuntimeStorage<'a, S, S2> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Memory(s) => s.read(buf).await.map_err(|_| Error::Io),
            Self::File(s) => s.read(buf).await.map_err(|_| Error::Io),
            Self::File2(s) => s.read(buf).await.map_err(|_| Error::Io),
            Self::None => Err(Error::NotReady),
        }
    }
}

impl<'a, S: FileStorage, S2: FileStorage> Seek for RuntimeStorage<'a, S, S2> {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match self {
            Self::Memory(s) => s.seek(pos).await.map_err(|_| Error::Io),
            Self::File(s) => s.seek(pos).await.map_err(|_| Error::Io),
            Self::File2(s) => s.seek(pos).await.map_err(|_| Error::Io),
            Self::None => Err(Error::NotReady),
        }
    }
}

impl<'a> MemoryStorage<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl<'a> ErrorType for MemoryStorage<'a> {
    type Error = core::convert::Infallible;
}

impl<'a> Read for MemoryStorage<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = cmp::min(buf.len(), self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl<'a> Seek for MemoryStorage<'a> {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::Current(n) => self.pos as i64 + n,
            SeekFrom::End(n) => self.data.len() as i64 + n,
        };
        self.pos = cmp::max(0, cmp::min(self.data.len() as i64, new_pos)) as usize;
        Ok(self.pos as u64)
    }
}
