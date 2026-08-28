use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
pub struct ReadOnlyFlash;
impl embedded_storage::nor_flash::ErrorType for ReadOnlyFlash { type Error = (); }
impl ReadNorFlash for ReadOnlyFlash {
    const READ_SIZE: usize = 1;
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> { Ok(()) }
    fn capacity(&self) -> usize { 512 * 1024 }
}
impl NorFlash for ReadOnlyFlash {
    const WRITE_SIZE: usize = 256;
    const ERASE_SIZE: usize = 4096;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> { Err(()) }
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> { Err(()) }
}
