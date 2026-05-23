//! Custom, lightweight asynchronous SD Card SPI Driver.
//!
//! Protocol notes:
//!  - All SPI transfers must keep MOSI at 0xFF when not sending a command or data token.
//!    SpiBus::read() sends 0x00 on MOSI which confuses the card; we always use
//!    transfer_in_place() with a 0xFF-prefilled buffer instead.
//!  - CS must be deasserted (high) with ≥8 dummy clocks between independent transactions.
//!  - The card must receive ≥74 clocks with CS high before the first CMD0.

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

    /// Send one 0xFF byte with CS high to flush the bus between transactions.
    async fn deselect_flush(&mut self) -> Result<(), Error> {
        self.cs.set_high().map_err(|_| Error::Cs)?;
        let mut dummy = [0xFF];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;
        Ok(())
    }

    /// Send a 6-byte SD command (CS already low) and return the R1 response byte.
    /// MOSI is kept 0xFF while polling for the response.
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

        // Wait for R1 response. SD spec allows up to Ncr = 8 bytes of 0xFF before
        // the card starts responding. We poll up to 10 times to be safe.
        for _ in 0..10 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if (byte[0] & 0x80) == 0 {
                return Ok(byte[0]);
            }
        }

        Ok(0xFF) // no response
    }

    /// Send CMD55 then the given app command. Both use CS already asserted.
    async fn send_app_cmd(&mut self, cmd: u8, arg: u32) -> Result<u8, Error> {
        let r1 = self.send_cmd(55, 0, 0xFF).await?;
        if r1 > 1 {
            // CMD55 must return 0x00 (ready) or 0x01 (idle) — anything else is an error.
            return Err(Error::Cmd55Failed);
        }
        self.send_cmd(cmd, arg, 0xFF).await
    }

    /// Receive `len` bytes from the card while keeping MOSI at 0xFF.
    /// Works in 64-byte chunks to stay on the stack.
    async fn rx_ff(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        let mut chunk_buf = [0xFF_u8; 64];
        let mut offset = 0;
        while offset < buf.len() {
            let n = (buf.len() - offset).min(64);
            // Pre-fill with 0xFF so MOSI is high during the transfer.
            for b in &mut chunk_buf[..n] { *b = 0xFF; }
            self.spi.transfer_in_place(&mut chunk_buf[..n]).await.map_err(|_| Error::Spi)?;
            buf[offset..offset + n].copy_from_slice(&chunk_buf[..n]);
            offset += n;
        }
        Ok(())
    }

    pub async fn init(&mut self) -> Result<(), Error> {
        // 1. Force initialization speed (≤400 kHz).
        self.spi.set_frequency(400_000);

        // 2. Wake-up sequence: CS high, ≥74 clock cycles (10 bytes = 80 clocks) with MOSI=0xFF.
        self.cs.set_high().map_err(|_| Error::Cs)?;
        let mut wake = [0xFF; 10];
        self.spi.transfer_in_place(&mut wake).await.map_err(|_| Error::Spi)?;

        // 3. Assert CS and send CMD0 (Go Idle State, software reset into SPI mode).
        //    Retry up to 10 times — some cards need a moment.
        self.cs.set_low().map_err(|_| Error::Cs)?;
        let mut r1 = 0xFF_u8;
        for attempt in 0..10 {
            r1 = self.send_cmd(0, 0, 0x95).await?;
            if r1 == 0x01 {
                break;
            }
            // Deselect, flush, reselect between CMD0 retries.
            self.deselect_flush().await?;
            Timer::after_millis(5).await;
            if attempt < 9 {
                self.cs.set_low().map_err(|_| Error::Cs)?;
            }
        }
        if r1 != 0x01 {
            self.cs.set_high().ok();
            return Err(Error::Timeout);
        }

        // Deselect between CMD0 and CMD8 (mandatory inter-command gap).
        self.deselect_flush().await?;
        self.cs.set_low().map_err(|_| Error::Cs)?;

        // 4. CMD8 (Send Interface Condition) — distinguishes v1 from v2 cards.
        let mut r1 = self.send_cmd(8, 0x1AA, 0x87).await?;
        let is_v2;
        if r1 == 0x01 {
            // v2 card: read 4 trailing R7 bytes (echo pattern must be 0xAA).
            is_v2 = true;
            let mut r7 = [0xFF; 4];
            self.rx_ff(&mut r7).await?;
            if r7[3] != 0xAA {
                self.cs.set_high().ok();
                return Err(Error::VoltageNotSupported);
            }
        } else if (r1 & 0x04) != 0 {
            // Illegal command → v1 card.
            is_v2 = false;
        } else {
            self.cs.set_high().ok();
            return Err(Error::Cmd8Failed);
        }

        // 5. ACMD41 loop: wait until card leaves idle (bit 0 of R1 clears).
        //    v2 cards need HCS=1 in the argument so they can report SDHC capacity.
        let acmd41_arg = if is_v2 { 0x4000_0000_u32 } else { 0 };
        r1 = 0xFF;
        for _ in 0..100 {
            // Each ACMD = CMD55 + the actual app command; deselect between them.
            self.deselect_flush().await?;
            self.cs.set_low().map_err(|_| Error::Cs)?;
            r1 = self.send_app_cmd(41, acmd41_arg).await?;
            if r1 == 0x00 {
                break;
            }
            Timer::after_millis(10).await;
        }
        if r1 != 0x00 {
            self.cs.set_high().ok();
            return Err(Error::InitTimeout);
        }

        // 6. CMD58: Read OCR to determine SDHC vs SDSC (block vs byte addressing).
        self.deselect_flush().await?;
        self.cs.set_low().map_err(|_| Error::Cs)?;
        let mut is_sdhc = false;
        if is_v2 {
            let r1 = self.send_cmd(58, 0, 0xFF).await?;
            if r1 == 0x00 {
                let mut ocr = [0xFF; 4];
                self.rx_ff(&mut ocr).await?;
                // Bit 30 (CCS) = 1 → SDHC/SDXC (block addressing).
                is_sdhc = (ocr[0] & 0x40) != 0;
            }
        }
        self.is_sdhc = is_sdhc;

        // 7. For SDSC: CMD16 forces the block length to 512 bytes.
        if !is_sdhc {
            self.deselect_flush().await?;
            self.cs.set_low().map_err(|_| Error::Cs)?;
            let r1 = self.send_cmd(16, 512, 0xFF).await?;
            if r1 != 0x00 {
                self.cs.set_high().ok();
                return Err(Error::Cmd16Failed);
            }
        }

        // 8. Read CSD to determine card capacity.
        self.deselect_flush().await?;
        self.cs.set_low().map_err(|_| Error::Cs)?;
        let num_blocks = self.read_csd_internal().await?;
        self.num_blocks = num_blocks;

        // 9. Final deselect.
        self.deselect_flush().await?;

        // 10. Scale up to data-transfer speed (4 MHz is conservative but reliable on prototypes).
        self.spi.set_frequency(4_000_000);

        defmt::info!("SD: init ok, SDHC={}, blocks={}", is_sdhc, num_blocks);
        Ok(())
    }

    /// Read the 16-byte CSD register and return the number of 512-byte blocks.
    /// CS must be low on entry; CS is left low on return.
    async fn read_csd_internal(&mut self) -> Result<u32, Error> {
        let r1 = self.send_cmd(9, 0, 0xFF).await?;
        if r1 != 0x00 {
            return Err(Error::ReadFailed(r1));
        }

        // Wait for the data token 0xFE.
        let token = self.wait_data_token().await?;
        if token != 0xFE {
            return Err(Error::TokenError(token));
        }

        let mut csd = [0u8; 16];
        self.rx_ff(&mut csd).await?;

        // Drain 2 CRC bytes (not checked in SPI mode by default).
        let mut crc = [0xFF; 2];
        self.spi.transfer_in_place(&mut crc).await.map_err(|_| Error::Spi)?;

        // Parse capacity.
        let num_blocks = if (csd[0] & 0xC0) == 0x40 {
            // CSD v2.0 — SDHC/SDXC
            let c_size = (((csd[7] & 0x3F) as u32) << 16)
                | ((csd[8] as u32) << 8)
                | (csd[9] as u32);
            (c_size + 1) * 1024
        } else if (csd[0] & 0xC0) == 0x00 {
            // CSD v1.0 — SDSC
            let c_size = (((csd[6] & 0x03) as u32) << 10)
                | ((csd[7] as u32) << 2)
                | (((csd[8] & 0xC0) as u32) >> 6);
            let c_size_mult = (((csd[9] & 0x03) as u32) << 1)
                | (((csd[10] & 0x80) as u32) >> 7);
            let read_bl_len = (csd[5] & 0x0F) as u32;
            let mult = 1u32 << (c_size_mult + 2);
            let block_len = 1u32 << read_bl_len;
            let total_bytes = (c_size + 1) * mult * block_len;
            total_bytes / 512
        } else {
            0
        };

        Ok(num_blocks)
    }

    /// Poll for a non-0xFF byte (data token or error token) from the card.
    /// MOSI is kept 0xFF throughout.
    async fn wait_data_token(&mut self) -> Result<u8, Error> {
        for _ in 0..2000 {
            let mut byte = [0xFF];
            self.spi.transfer_in_place(&mut byte).await.map_err(|_| Error::Spi)?;
            if byte[0] != 0xFF {
                return Ok(byte[0]);
            }
        }
        Err(Error::Timeout)
    }

    pub async fn read_block(&mut self, block_address: u32, buf: &mut [u8; 512]) -> Result<(), Error> {
        // SDHC uses block addresses; SDSC uses byte addresses.
        let addr = if self.is_sdhc { block_address } else { block_address * 512 };

        self.cs.set_low().map_err(|_| Error::Cs)?;

        let r1 = self.send_cmd(17, addr, 0xFF).await?;
        if r1 != 0x00 {
            self.deselect_flush().await.ok();
            return Err(Error::ReadFailed(r1));
        }

        // Wait for start-of-data token 0xFE.
        let token = self.wait_data_token().await?;
        if token != 0xFE {
            self.deselect_flush().await.ok();
            return Err(Error::TokenError(token));
        }

        // Read 512 bytes — MOSI must stay 0xFF.
        self.rx_ff(buf).await?;

        // Drain 2 CRC bytes.
        let mut crc = [0xFF; 2];
        self.spi.transfer_in_place(&mut crc).await.map_err(|_| Error::Spi)?;

        // Deselect + flush.
        self.deselect_flush().await?;

        Ok(())
    }

    pub async fn write_block(&mut self, block_address: u32, buf: &[u8; 512]) -> Result<(), Error> {
        let addr = if self.is_sdhc { block_address } else { block_address * 512 };

        self.cs.set_low().map_err(|_| Error::Cs)?;

        let r1 = self.send_cmd(24, addr, 0xFF).await?;
        if r1 != 0x00 {
            self.deselect_flush().await.ok();
            return Err(Error::WriteFailed(r1));
        }

        // One dummy byte before the data token (required by SD spec §7.3.3.1).
        let mut dummy = [0xFF];
        self.spi.transfer_in_place(&mut dummy).await.map_err(|_| Error::Spi)?;

        // Start block token.
        self.spi.write(&[0xFE]).await.map_err(|_| Error::Spi)?;

        // Send 512 data bytes.
        self.spi.write(buf).await.map_err(|_| Error::Spi)?;

        // Two dummy CRC bytes.
        self.spi.write(&[0xFF, 0xFF]).await.map_err(|_| Error::Spi)?;

        // Read data response token (bits [4:1] = 0b0101 means accepted).
        let mut response = [0xFF];
        self.spi.transfer_in_place(&mut response).await.map_err(|_| Error::Spi)?;
        if (response[0] & 0x1F) != 0x05 {
            self.deselect_flush().await.ok();
            return Err(Error::WriteRejected(response[0]));
        }

        // Wait while card is busy writing (MISO low = still busy).
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
            self.deselect_flush().await.ok();
            return Err(Error::BusyTimeout);
        }

        // Deselect + flush.
        self.deselect_flush().await?;

        Ok(())
    }
}
