use core::cmp;
use embedded_io_async::{Read, Seek, SeekFrom};

pub struct MemoryStorage<'a> {
    data: &'a [u8],
    pos: usize,
}

#[derive(Clone, Copy, Debug)]
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
                // Check if the rest of entry.filename is padding (0xA0)
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

impl<'a> MemoryStorage<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

impl<'a> embedded_io_async::ErrorType for MemoryStorage<'a> {
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
