use core::cmp::min;
use embedded_hal::spi::SpiBus;
use embedded_hal::digital::OutputPin;

#[derive(Debug, defmt::Format)]
pub enum SdError {
    Timeout,
    CommandError(u8),
    WriteError,
    SpiError,
    CsError,
}

pub trait SpiFreq {
    fn set_freq(&mut self, freq: u32);
}

pub struct SdCard<Spi, Cs, Delay> {
    pub spi: Spi,
    cs: Cs,
    delay: Delay,
    is_sdhc: bool,
    pub initialized: bool,
    baudrate: u32,
}

impl<Spi, Cs, Delay> SdCard<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq,
    Cs: OutputPin,
    Delay: embedded_hal::delay::DelayNs,
{
    pub fn new(spi: Spi, cs: Cs, delay: Delay, baudrate: u32) -> Self {
        Self {
            spi,
            cs,
            delay,
            is_sdhc: false,
            initialized: false,
            baudrate,
        }
    }

    pub fn init(&mut self) -> Result<(), SdError> {
        self.spi.set_freq(400_000);
        // 74+ dummy clocks with CS high
        self.cs.set_high().map_err(|_| SdError::CsError)?;
        self.delay.delay_us(1000);
        for _ in 0..10 {
            self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
        }
        
        self.cs.set_low().map_err(|_| SdError::CsError)?;

        let res = (|| {
            // CMD0: GO_IDLE_STATE
            let mut r1 = 0xFF;
            let mut attempts = 100;
            while attempts > 0 {
                match self.send_command(0, 0, 0x95) {
                    Ok(0x01) => {
                        r1 = 0x01;
                        break;
                    }
                    Ok(res) => r1 = res,
                    Err(_) => {}
                }
                
                self.cs.set_high().map_err(|_| SdError::CsError)?;
                for _ in 0..8 {
                    self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
                }
                self.cs.set_low().map_err(|_| SdError::CsError)?;

                attempts -= 1;
                self.delay.delay_us(1000);
            }
            
            if r1 != 0x01 {
                return Err(SdError::CommandError(r1));
            }

            self.cs.set_high().map_err(|_| SdError::CsError)?;
            self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
            self.cs.set_low().map_err(|_| SdError::CsError)?;

            // CMD8
            let r1 = self.send_command(8, 0x000001AA, 0x87)?;
            if r1 == 0x01 {
                let mut r7 = [0u8; 4];
                self.spi_read(&mut r7)?;
                if r7[2] != 0x01 || r7[3] != 0xAA {
                    return Err(SdError::CommandError(r1));
                }
            } else if r1 != 0x05 { // 0x05 means illegal command (older SD cards)
                return Err(SdError::CommandError(r1));
            }

            self.cs.set_high().map_err(|_| SdError::CsError)?;
            self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
            self.cs.set_low().map_err(|_| SdError::CsError)?;
            
            // ACMD41: SD_SEND_OP_COND
            let mut retries = 10000;
            loop {
                let r1 = self.send_command(55, 0, 0xFF)?;
                if r1 != 0x01 && r1 != 0x00 {
                    return Err(SdError::CommandError(r1));
                }
                
                self.cs.set_high().map_err(|_| SdError::CsError)?;
                self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
                self.cs.set_low().map_err(|_| SdError::CsError)?;

                let r1 = self.send_command(41, 0x40000000, 0xFF)?;
                if r1 == 0x00 {
                    break;
                }
                
                self.cs.set_high().map_err(|_| SdError::CsError)?;
                self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
                self.cs.set_low().map_err(|_| SdError::CsError)?;

                retries -= 1;
                if retries == 0 {
                    return Err(SdError::Timeout);
                }
            }

            self.cs.set_high().map_err(|_| SdError::CsError)?;
            self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;
            self.cs.set_low().map_err(|_| SdError::CsError)?;

            // CMD58: READ_OCR
            let r1 = self.send_command(58, 0, 0xFD)?;
            if r1 == 0x00 {
                let mut ocr = [0u8; 4];
                self.spi_read(&mut ocr)?;
                self.is_sdhc = (ocr[0] & 0x40) != 0;
            } else {
                return Err(SdError::CommandError(r1));
            }

            Ok(())
        })();

        self.cs.set_high().map_err(|_| SdError::CsError)?;
        self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;

        if res.is_ok() {
            self.spi.set_freq(self.baudrate);
            self.initialized = true;
        }

        res
    }

    fn spi_read(&mut self, buf: &mut [u8]) -> Result<(), SdError> {
        buf.fill(0xFF);
        self.spi.transfer_in_place(buf).map_err(|_| SdError::SpiError)?;
        Ok(())
    }

    fn wait_not_busy(&mut self) -> Result<(), SdError> {
        let mut retries = 100000;
        let mut res = [0xFF];
        loop {
            self.spi_read(&mut res)?;
            if res[0] == 0xFF {
                return Ok(());
            }
            retries -= 1;
            if retries == 0 {
                return Err(SdError::Timeout);
            }
        }
    }

    fn send_command(&mut self, cmd: u8, arg: u32, crc: u8) -> Result<u8, SdError> {
        if cmd != 0 {
            self.wait_not_busy()?;
        }

        let buf = [
            0x40 | cmd,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            arg as u8,
            crc,
        ];
        
        self.spi.write(&buf).map_err(|_| SdError::SpiError)?;

        let mut retries = 100;
        let mut res = [0xFF];
        while retries > 0 {
            self.spi_read(&mut res)?;
            if (res[0] & 0x80) == 0 {
                return Ok(res[0]);
            }
            retries -= 1;
        }
        
        Err(SdError::Timeout)
    }

    pub fn read_block(&mut self, block: u32, buffer: &mut [u8]) -> Result<(), SdError> {
        if !self.initialized {
            self.init()?;
        }

        if buffer.len() < 512 {
            return Err(SdError::CommandError(0));
        }

        let addr = if self.is_sdhc { block } else { block * 512 };

        self.cs.set_low().map_err(|_| SdError::CsError)?;

        let res = (|| {
            let r1 = self.send_command(17, addr, 0xFF)?;
            if r1 != 0x00 {
                return Err(SdError::CommandError(r1));
            }

            let mut retries = 100000;
            let mut res = [0xFF];
            loop {
                self.spi_read(&mut res)?;
                if res[0] == 0xFE {
                    break;
                }
                retries -= 1;
                if retries == 0 {
                    return Err(SdError::Timeout);
                }
            }

            self.spi_read(&mut buffer[..512])?;

            let mut crc = [0xFF; 2];
            self.spi_read(&mut crc)?;

            Ok(())
        })();

        self.cs.set_high().map_err(|_| SdError::CsError)?;
        self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;

        if res.is_err() {
            self.initialized = false;
        }

        res
    }

    pub fn write_block(&mut self, block: u32, buffer: &[u8]) -> Result<(), SdError> {
        if !self.initialized {
            self.init()?;
        }

        if buffer.len() < 512 {
            return Err(SdError::CommandError(0));
        }

        let addr = if self.is_sdhc { block } else { block * 512 };

        self.cs.set_low().map_err(|_| SdError::CsError)?;

        let res = (|| {
            let r1 = self.send_command(24, addr, 0xFF)?;
            if r1 != 0x00 {
                return Err(SdError::CommandError(r1));
            }

            self.spi.write(&[0xFF, 0xFE]).map_err(|_| SdError::SpiError)?;
            self.spi.write(&buffer[..512]).map_err(|_| SdError::SpiError)?;
            self.spi.write(&[0xFF, 0xFF]).map_err(|_| SdError::SpiError)?;

            let mut res = [0xFF];
            self.spi_read(&mut res)?;
            if (res[0] & 0x1F) != 0x05 {
                return Err(SdError::WriteError);
            }

            let mut retries = 1000000;
            loop {
                self.spi_read(&mut res)?;
                if res[0] != 0x00 {
                    break;
                }
                retries -= 1;
                if retries == 0 {
                    return Err(SdError::Timeout);
                }
            }

            Ok(())
        })();

        self.cs.set_high().map_err(|_| SdError::CsError)?;
        self.spi.write(&[0xFF]).map_err(|_| SdError::SpiError)?;

        if res.is_err() {
            self.initialized = false;
        }

        res
    }
}

pub struct SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    card_ptr: *mut SdCard<Spi, Cs, Delay>,
    offset: u64,
    cache: [u8; 512],
    cached_sector: Option<u32>,
}

unsafe impl<Spi, Cs, Delay> Send for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}
unsafe impl<Spi, Cs, Delay> Sync for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}

impl<Spi, Cs, Delay> Clone for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
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

impl<Spi, Cs, Delay> Copy for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{}

impl<Spi, Cs, Delay> SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    pub fn new(card_ptr: *mut SdCard<Spi, Cs, Delay>) -> Self {
        Self {
            card_ptr,
            offset: 0,
            cache: [0u8; 512],
            cached_sector: None,
        }
    }

    pub fn reset(&mut self) {
        self.cached_sector = None;
        let card = unsafe { &mut *self.card_ptr };
        card.initialized = false;
    }
}

impl<Spi, Cs, Delay> fatfs::IoBase for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    type Error = fatfs::Error<()>;
}

impl<Spi, Cs, Delay> fatfs::Read for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut total_read = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_read < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = min(len - total_read, bytes_avail);
            
            if self.cached_sector != Some(sector) {
                if let Err(e) = card.read_block(sector, &mut self.cache) {
                    defmt::error!("SdBlockIo: read block failed: {:?}", defmt::Debug2Format(&e));
                    return Err(fatfs::Error::Io(()));
                }
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

impl<Spi, Cs, Delay> fatfs::Seek for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        let new_offset = match pos {
            fatfs::SeekFrom::Start(n) => n,
            fatfs::SeekFrom::Current(n) => (self.offset as i64 + n) as u64,
            fatfs::SeekFrom::End(n) => {
                let total_bytes = 4_000_000_000u64;
                (total_bytes as i64 + n) as u64
            }
        };
        self.offset = new_offset;
        Ok(new_offset)
    }
}

impl<Spi, Cs, Delay> fatfs::Write for SdBlockIo<Spi, Cs, Delay>
where
    Spi: SpiBus<u8> + SpiFreq + 'static,
    Cs: OutputPin + 'static,
    Delay: embedded_hal::delay::DelayNs + 'static,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut total_written = 0;
        let len = buf.len();
        let card = unsafe { &mut *self.card_ptr };
        
        while total_written < len {
            let sector = (self.offset / 512) as u32;
            let offset_in_sector = (self.offset % 512) as usize;
            let bytes_avail = 512 - offset_in_sector;
            let to_copy = min(len - total_written, bytes_avail);

            if self.cached_sector != Some(sector) {
                if to_copy < 512 {
                    if card.read_block(sector, &mut self.cache).is_err() {
                        self.cache = [0u8; 512];
                    }
                }
                self.cached_sector = Some(sector);
            }

            self.cache[offset_in_sector..offset_in_sector + to_copy]
                .copy_from_slice(&buf[total_written..total_written + to_copy]);
            
            if let Err(e) = card.write_block(sector, &self.cache) {
                defmt::error!("SdBlockIo: write block failed: {:?}", defmt::Debug2Format(&e));
                return Err(fatfs::Error::Io(()));
            }

            self.offset += to_copy as u64;
            total_written += to_copy;
        }
        Ok(total_written)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
