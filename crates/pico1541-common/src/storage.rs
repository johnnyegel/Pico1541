use core::cmp;
use embedded_io_async::{Read, Seek, SeekFrom, ErrorType};

pub struct MemoryStorage<'a> {
    data: &'a [u8],
    pos: usize,
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
                for &e_byte in entry.filename.iter().skip(filename.len()).take(16 - filename.len()) {
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

pub enum RuntimeStorage<'a, S: FileStorage> {
    Memory(MemoryStorage<'a>),
    File(S),
    None,
}

impl<'a, S: FileStorage> ErrorType for RuntimeStorage<'a, S> {
    type Error = Error;
}

impl<'a, S: FileStorage> Read for RuntimeStorage<'a, S> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Memory(s) => s.read(buf).await.map_err(|_| Error::Io),
            Self::File(s) => s.read(buf).await.map_err(|_| Error::Io),
            Self::None => Err(Error::NotReady),
        }
    }
}

impl<'a, S: FileStorage> Seek for RuntimeStorage<'a, S> {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match self {
            Self::Memory(s) => s.seek(pos).await.map_err(|_| Error::Io),
            Self::File(s) => s.seek(pos).await.map_err(|_| Error::Io),
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
