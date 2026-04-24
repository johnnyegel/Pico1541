use pico1541_manager::{StorageDriver, Error};

pub struct SdStorage;

impl SdStorage {
    pub fn new() -> Self {
        Self
    }
}

impl StorageDriver for SdStorage {
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), Error> {
        // TODO: Implement SD Card read using GPIO 11-15
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), Error> {
        // TODO: Implement SD Card write using GPIO 11-15
        Ok(())
    }
}
