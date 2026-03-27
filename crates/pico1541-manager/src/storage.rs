use embedded_io_async::{Read, Seek, SeekFrom};
use core::cmp;

pub struct MemoryStorage<'a> {
    data: &'a [u8],
    pos: usize,
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
