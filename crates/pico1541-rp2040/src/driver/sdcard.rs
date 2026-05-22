//! Custom, lightweight asynchronous SD Card SPI Driver.

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiBus;
use embassy_time::Timer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Error {
    Spi,
    Cs,
    Timeout,
    VoltageNotSupported,
    Cmd8Failed,
    Cmd55Failed,
    InitTimeout,
    Cmd16Failed,
    ReadFailed(u8),
    WriteFailed(u8),
    TokenError(u8),
    WriteRejected(u8),
    BusyTimeout,
}

pub trait SdSpi {
    fn set_frequency(&mut self, freq: u32);
}

impl<T: embassy_rp::spi::Instance, M: embassy_rp::spi::Mode> SdSpi for embassy_rp::spi::Spi<'static, T, M> {
    fn set_frequency(&mut self, freq: u32) {
        self.set_frequency(freq);
    }
}

pub struct SdCard<SPI, CS> {
    spi: SPI,
    cs: CS,
    is_sdhc: bool,
    num_blocks: u32,
}

impl<SPI, CS> SdCard<SPI, CS>
where
    SPI: SpiBus<u8> + SdSpi,
    CS: OutputPin,
{
    pub fn new(spi: SPI, cs: CS) -> Self {
        Self {
            spi,
            cs,
            is_sdhc: false,
            num_blocks: 0,
        }
    }

    /// Free underlying SPI and CS pins
    #[allow(dead_code)]
    pub fn free(self) -> (SPI, CS) {
        (self.spi, self.cs)
    }

    #[allow(dead_code)]
    pub fn is_sdhc(&self) -> bool {
        self.is_sdhc
    }

    pub fn num_blocks(&self) -> u32 {
        self.num_blocks
    }

    #[allow(dead_code)]
    pub fn spi_mut(&mut self) -> &mut SPI {
        &mut self.spi
    }

    async fn send_cmd(&mut self, cmd: u8, arg: u32, crc: u8) -> Result<u8, Error> {
        let cmd_bytes = [
            0x40 | cmd,
            (arg >> 24) as u8,
            (arg >> 16) as u8,
            (arg >> 8) as u8,
            arg as u8,
            crc | 0x01,
        ];
        
        self.spi.write(&cmd_bytes).await.map_err(|_| Error::Spi)?;
        
        // Wait for response (MSB is 0). Try up to 8 times.
        for _ in 0..8 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if (byte[0] & 0x80) == 0 {
                return Ok(byte[0]);
            }
        }
        
        // Return 0xFF if no valid response within 8 bytes
        Ok(0xFF)
    }

    async fn send_app_cmd(&mut self, cmd: u8, arg: u32) -> Result<u8, Error> {
        let r1 = self.send_cmd(55, 0, 0xFF).await?;
        if r1 > 1 {
            return Err(Error::Cmd55Failed);
        }
        self.send_cmd(cmd, arg, 0xFF).await
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        // 1. Force low speed (400kHz) for card initialization
        self.spi.set_frequency(400_000);

        // 2. Wake up card: CS high, send at least 74 clock cycles (we send 10 bytes = 80 clocks)
        self.cs.set_high().map_err(|_| Error::Cs)?;
        let mut dummy = [0xFF; 10];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;

        // 3. Select card (CS low)
        self.cs.set_low().map_err(|_| Error::Cs)?;

        // 4. Send CMD0 (Go Idle State) until response is 0x01
        let mut r1 = 0xFF;
        for _ in 0..100 {
            r1 = self.send_cmd(0, 0, 0x95).await?;
            if r1 == 0x01 {
                break;
            }
            Timer::after_millis(10).await;
        }
        if r1 != 0x01 {
            self.cs.set_high().ok();
            return Err(Error::Timeout);
        }

        // 5. Send CMD8 (Send Interface Condition) to query voltage and check v1 vs v2
        let r1 = self.send_cmd(8, 0x1AA, 0x87).await?;
        let is_v2;
        if r1 == 0x01 {
            is_v2 = true;
            let mut r7 = [0xFF; 4];
            self.spi.transfer_in_place(&mut r7).await.map_err(|_| Error::Spi)?;
            if r7[3] != 0xAA {
                self.cs.set_high().ok();
                return Err(Error::VoltageNotSupported);
            }
        } else if (r1 & 0x04) != 0 {
            // Illegal command response -> v1 card
            is_v2 = false;
        } else {
            self.cs.set_high().ok();
            return Err(Error::Cmd8Failed);
        }

        // 6. Send ACMD41 (SD_SEND_OP_COND) in a loop until the card leaves idle state (response 0x00)
        let mut r1 = 0xFF;
        let arg = if is_v2 { 0x4000_0000 } else { 0 };
        for _ in 0..100 {
            r1 = self.send_app_cmd(41, arg).await?;
            if r1 == 0x00 {
                break;
            }
            Timer::after_millis(10).await;
        }
        if r1 != 0x00 {
            self.cs.set_high().ok();
            return Err(Error::InitTimeout);
        }

        // 7. If v2, send CMD58 (Read OCR) to check if card is SDHC/SDXC (block addressing)
        let mut is_sdhc = false;
        if is_v2 {
            let r1 = self.send_cmd(58, 0, 0xFF).await?;
            if r1 == 0x00 {
                let mut ocr = [0xFF; 4];
                self.spi.transfer_in_place(&mut ocr).await.map_err(|_| Error::Spi)?;
                if (ocr[0] & 0x40) != 0 {
                    is_sdhc = true;
                }
            }
        }
        self.is_sdhc = is_sdhc;

        // 8. For SDSC cards, force block length to 512 bytes using CMD16
        if !is_sdhc {
            let r1 = self.send_cmd(16, 512, 0xFF).await?;
            if r1 != 0x00 {
                self.cs.set_high().ok();
                return Err(Error::Cmd16Failed);
            }
        }

        // 9. Read and parse CSD to calculate capacity (num_blocks)
        let num_blocks = self.read_csd_internal().await?;
        self.num_blocks = num_blocks;

        // 10. Deselect card
        self.cs.set_high().map_err(|_| Error::Cs)?;
        let mut dummy = [0xFF; 1];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;

        // 11. Scale up to high speed (4MHz) after successful initialization
        self.spi.set_frequency(4_000_000);

        Ok(())
    }

    async fn read_csd_internal(&mut self) -> Result<u32, Error> {
        let r1 = self.send_cmd(9, 0, 0xFF).await?;
        if r1 != 0x00 {
            return Err(Error::ReadFailed(r1));
        }
        
        let mut token = 0xFF;
        for _ in 0..2000 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if byte[0] != 0xFF {
                token = byte[0];
                break;
            }
        }
        if token != 0xFE {
            return Err(Error::TokenError(token));
        }
        
        let mut csd = [0u8; 16];
        self.spi.read(&mut csd).await.map_err(|_| Error::Spi)?;
        
        let mut crc = [0xFF; 2];
        self.spi.transfer_in_place(&mut crc).await.map_err(|_| Error::Spi)?;
        
        // Parse size
        let num_blocks = if (csd[0] & 0xC0) == 0x40 {
            // CSD Version 2.0 (High Capacity / SDHC)
            let c_size = (((csd[7] & 0x3F) as u32) << 16)
                | ((csd[8] as u32) << 8)
                | (csd[9] as u32);
            (c_size + 1) * 1024
        } else if (csd[0] & 0xC0) == 0 {
            // CSD Version 1.0 (Standard Capacity / SDSC)
            let c_size = (((csd[6] & 0x03) as u32) << 10)
                | ((csd[7] as u32) << 2)
                | (((csd[8] & 0xC0) as u32) >> 6);
            let c_size_mult = (((csd[9] & 0x03) as u32) << 1)
                | (((csd[10] & 0x80) as u32) >> 7);
            let read_bl_len = (csd[5] & 0x0F) as u32;
            let mult = 1 << (c_size_mult + 2);
            let block_len = 1 << read_bl_len;
            let total_bytes = (c_size + 1) * mult * block_len;
            total_bytes / 512
        } else {
            0
        };
        
        Ok(num_blocks)
    }

    pub async fn read_block(&mut self, block_address: u32, buf: &mut [u8; 512]) -> Result<(), Error> {
        let addr = if self.is_sdhc { block_address } else { block_address * 512 };
        
        self.cs.set_low().map_err(|_| Error::Cs)?;
        
        let r1 = self.send_cmd(17, addr, 0xFF).await?;
        if r1 != 0x00 {
            self.cs.set_high().ok();
            return Err(Error::ReadFailed(r1));
        }
        
        // Wait for data token (0xFE)
        let mut token = 0xFF;
        for _ in 0..2000 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if byte[0] != 0xFF {
                token = byte[0];
                break;
            }
        }
        
        if token != 0xFE {
            self.cs.set_high().ok();
            return Err(Error::TokenError(token));
        }
        
        // Read 512 bytes of data
        self.spi.read(buf).await.map_err(|_| Error::Spi)?;
        
        // Read 2 bytes CRC
        let mut crc = [0xFF; 2];
        self.spi.transfer_in_place(&mut crc).await.map_err(|_| Error::Spi)?;
        
        self.cs.set_high().map_err(|_| Error::Cs)?;
        
        // Send 8 dummy clocks
        let mut dummy = [0xFF; 1];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;
        
        Ok(())
    }

    pub async fn write_block(&mut self, block_address: u32, buf: &[u8; 512]) -> Result<(), Error> {
        let addr = if self.is_sdhc { block_address } else { block_address * 512 };
        
        self.cs.set_low().map_err(|_| Error::Cs)?;
        
        let r1 = self.send_cmd(24, addr, 0xFF).await?;
        if r1 != 0x00 {
            self.cs.set_high().ok();
            return Err(Error::WriteFailed(r1));
        }
        
        // Send dummy byte
        let mut dummy = [0xFF];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;
        
        // Send Start Block token (0xFE)
        let mut start_token = [0xFE];
        self.spi.transfer_in_place(&mut start_token).await.map_err(|_| Error::Spi)?;
        
        // Send 512 bytes data
        self.spi.write(buf).await.map_err(|_| Error::Spi)?;
        
        // Send 2 dummy CRC bytes
        let mut dummy_crc = [0xFF; 2];
        self.spi.transfer_in_place(&mut dummy_crc).await.map_err(|_| Error::Spi)?;
        
        // Read data response token
        let mut response = [0xFF];
        self.spi.transfer_in_place(&mut response).await.map_err(|_| Error::Spi)?;
        if (response[0] & 0x1F) != 0x05 {
            self.cs.set_high().ok();
            return Err(Error::WriteRejected(response[0]));
        }
        
        // Wait while card is busy (MISO is held low)
        let mut busy = true;
        for _ in 0..5000 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if byte[0] == 0xFF {
                busy = false;
                break;
            }
            Timer::after_micros(100).await;
        }
        
        if busy {
            self.cs.set_high().ok();
            return Err(Error::BusyTimeout);
        }
        
        self.cs.set_high().map_err(|_| Error::Cs)?;
        
        // Send 8 dummy clocks
        let mut dummy = [0xFF; 1];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;
        
        Ok(())
    }
}
