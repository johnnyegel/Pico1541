use embassy_rp::flash::{Flash, Blocking};
use embassy_rp::peripherals::FLASH;
use littlefs_rust::Storage;
use littlefs_rust::Error;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};


// The RP2040 has 2MB of flash. We reserved 512KB at the end of flash.
pub const FS_SIZE: usize = 512 * 1024;
pub const FS_OFFSET: usize = 1536 * 1024;
// embassy-rp flash constants
pub const FLASH_PAGE_SIZE: usize = 256;
pub const FLASH_SECTOR_SIZE: usize = 4096;

pub struct FlashStorage<'d> {
    flash: Flash<'d, FLASH, Blocking, { 2 * 1024 * 1024 }>,
}

impl<'d> FlashStorage<'d> {
    pub fn new(flash: Flash<'d, FLASH, Blocking, { 2 * 1024 * 1024 }>) -> Self {
        Self { flash }
    }
}

impl<'d> Storage for FlashStorage<'d> {
    fn read(&mut self, block: u32, offset: u32, buf: &mut [u8]) -> Result<(), Error> {
        let abs_addr = (FS_OFFSET as u32) + (block * (FLASH_SECTOR_SIZE as u32)) + offset;
        self.flash.read(abs_addr, buf).map_err(|_| Error::Io)
    }

    fn write(&mut self, block: u32, offset: u32, data: &[u8]) -> Result<(), Error> {
        let abs_addr = (FS_OFFSET as u32) + (block * (FLASH_SECTOR_SIZE as u32)) + offset;
        self.flash.write(abs_addr, data).map_err(|_| Error::Io)
    }

    fn erase(&mut self, block: u32) -> Result<(), Error> {
        let abs_addr = (FS_OFFSET as u32) + (block * (FLASH_SECTOR_SIZE as u32));
        self.flash.erase(abs_addr, abs_addr + (FLASH_SECTOR_SIZE as u32)).map_err(|_| Error::Io)
    }

    fn sync(&mut self) -> Result<(), Error> {
        Ok(())
    }
}




