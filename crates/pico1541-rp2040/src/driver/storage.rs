use embedded_sdmmc::{SdCard, Block, BlockIdx, BlockDevice};

pub struct SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    card_ptr: *mut SdCard<SpiDev, Delay>,
    offset: u64,
    cache: [u8; 512],
    cached_sector: Option<u32>,
}

unsafe impl<SpiDev, Delay> Send for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}
unsafe impl<SpiDev, Delay> Sync for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}

impl<SpiDev, Delay> Clone for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    fn clone(&self) -> Self {
        Self {
            card_ptr: self.card_ptr,
            offset: self.offset,
            cache: self.cache,
            cached_sector: self.cached_sector,
        }
    }
}

impl<SpiDev, Delay> Copy for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}

impl<SpiDev, Delay> SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    pub fn new(card_ptr: *mut SdCard<SpiDev, Delay>) -> Self {
        Self {
            card_ptr,
            offset: 0,
            cache: [0u8; 512],
            cached_sector: None,
        }
    }
}

impl<SpiDev, Delay> hadris_io::r#async::Read for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, hadris_io::Error> {
        let mut total_read = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_read < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_read).min(bytes_avail);
            
            if self.cached_sector != Some(sector) {
                let mut read_blocks = [Block { contents: [0u8; 512] }];
                card.read(&mut read_blocks, BlockIdx(sector)).map_err(|e| {
                    defmt::error!("SdBlockIo: read block failed: {:?}", defmt::Debug2Format(&e));
                    hadris_io::Error::new_static(hadris_io::ErrorKind::Other, "SD read error")
                })?;
                self.cache = read_blocks[0].contents;
                self.cached_sector = Some(sector);
            }
            
            let src = &self.cache[offset_in_sector..offset_in_sector + to_copy];
            buf[total_read..total_read + to_copy].copy_from_slice(src);
            
            self.offset += to_copy as u64;
            total_read += to_copy;
        }
        
        Ok(total_read)
    }
}

impl<SpiDev, Delay> hadris_io::r#async::Seek for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    async fn seek(&mut self, pos: hadris_io::SeekFrom) -> Result<u64, hadris_io::Error> {
        let card = unsafe { &mut *self.card_ptr };
        let new_offset = match pos {
            hadris_io::SeekFrom::Start(n) => n,
            hadris_io::SeekFrom::Current(n) => (self.offset as i64 + n) as u64,
            hadris_io::SeekFrom::End(n) => {
                let count = card.num_blocks().map_err(|e| {
                    defmt::error!("SdBlockIo: num_blocks failed: {:?}", defmt::Debug2Format(&e));
                    hadris_io::Error::new_static(hadris_io::ErrorKind::Other, "SD block count error")
                })?;
                let total_bytes = count.0 as u64 * 512;
                (total_bytes as i64 + n) as u64
            }
        };
        self.offset = new_offset;
        Ok(new_offset)
    }
}

impl<SpiDev, Delay> hadris_io::r#async::Write for SdBlockIo<SpiDev, Delay>
where
    SpiDev: embedded_hal::spi::SpiDevice<u8> + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    async fn write(&mut self, buf: &[u8]) -> Result<usize, hadris_io::Error> {
        let mut total_written = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_written < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = (len - total_written).min(bytes_avail);

            if self.cached_sector != Some(sector) {
                let mut read_blocks = [Block { contents: [0u8; 512] }];
                if card.read(&mut read_blocks, BlockIdx(sector)).is_ok() {
                    self.cache = read_blocks[0].contents;
                } else {
                    self.cache = [0u8; 512];
                }
                self.cached_sector = Some(sector);
            }

            self.cache[offset_in_sector..offset_in_sector + to_copy]
                .copy_from_slice(&buf[total_written..total_written + to_copy]);
            
            let write_blocks = [Block { contents: self.cache }];
            card.write(&write_blocks, BlockIdx(sector)).map_err(|e| {
                defmt::error!("SdBlockIo: write block failed: {:?}", defmt::Debug2Format(&e));
                hadris_io::Error::new_static(hadris_io::ErrorKind::Other, "SD write error")
            })?;

            self.offset += to_copy as u64;
            total_written += to_copy;
        }
        Ok(total_written)
    }

    async fn flush(&mut self) -> Result<(), hadris_io::Error> {
        Ok(())
    }
}
