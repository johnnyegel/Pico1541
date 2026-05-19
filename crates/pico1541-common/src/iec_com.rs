use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IecError {
    Timeout,
    BusError,
}

impl fmt::Display for IecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => write!(f, "IEC Timeout"),
            Self::BusError => write!(f, "IEC Bus Error"),
        }
    }
}

pub const IEC_LISTEN: u8 = 0x20;
pub const IEC_TALK: u8 = 0x40;
pub const IEC_UNLISTEN: u8 = 0x3F;
pub const IEC_UNTALK: u8 = 0x5F;

pub const DELAY_POLL_US: u32 = 10;
pub const DELAY_RELEASE_US: u32 = 20;
pub const DELAY_DATA_US: u32 = 60;
pub const DELAY_EOI_TIMEOUT_US: u32 = 200;
pub const DELAY_ACK_TIMEOUT_US: u32 = 10_000_000;
pub const DELAY_HOLD_TIMEOUT_US: u32 = 10_000_000;

/// Hardware abstraction for the IEC bus.
///
/// All signals use the logical convention: **true = asserted**.
/// On the physical IEC bus an asserted line is pulled LOW.
#[allow(async_fn_in_trait)]
pub trait IecComHostBus {
    fn set_atn(&mut self, assert: bool);
    fn set_clk(&mut self, assert: bool);
    fn set_data(&mut self, assert: bool);
    fn get_clk(&self) -> bool;
    fn get_data(&self) -> bool;

    /// Optional method for tracing the internal state of the bus/drive during waits. Useful for debugging timeouts.
    fn trace_state(&self, tag: &str) -> ();

    async fn delay_us(&mut self, us: u32);

    async fn wait_for_clk(&mut self, assert: bool, timeout_us: u32) -> Result<(), IecError> {
        let mut elapsed = 0;
        self.trace_state(if assert { "wait_clk:assert" } else { "wait_clk:release" });
        while self.get_clk() != assert {
            if elapsed >= timeout_us {
                self.trace_state("wait_clk:timeout");
                log::trace!(
                    "wait_for_clk [{}] timeout ({}): got {}",
                    assert,
                    timeout_us,
                    self.get_clk()
                );
                return Err(IecError::Timeout);
            }
            self.delay_us(DELAY_POLL_US).await;
            elapsed += DELAY_POLL_US;
        }
        log::trace!("wait_for_clk [{}] success after {}us", assert, elapsed);
        Ok(())
    }

    async fn wait_for_data(&mut self, assert: bool, timeout_us: u32) -> Result<(), IecError> {
        let mut elapsed = 0;
        self.trace_state(if assert { "wait_data:assert" } else { "wait_data:release" });
        while self.get_data() != assert {
            if elapsed >= timeout_us {
                self.trace_state("wait_data:timeout");
                log::trace!(
                    "wait_for_data [{}] timeout ({}): got {}",
                    assert,
                    timeout_us,
                    self.get_data()
                );
                return Err(IecError::Timeout);
            }
            self.delay_us(DELAY_POLL_US).await;
            elapsed += DELAY_POLL_US;
        }
        log::trace!("wait_for_data [{}] success after {}us", assert, elapsed);
        Ok(())
    }
}

/// Universal IEC protocol tool acting as a Host (e.g. C64).
pub struct IecTransaction<'a, B: IecComHostBus> {
    bus: &'a mut B,
}

impl<'a, B: IecComHostBus> IecTransaction<'a, B> {
    pub fn new(bus: &'a mut B) -> Self {
        Self { bus }
    }

    /// Pulls ATN low and waits for the drive to acknowledge.
    pub async fn atn_assert(self) -> Result<Self, IecError> {
        self.bus.trace_state("atn_assert:start");

        // Assert ATN to start communication
        self.bus.set_atn(true);

        // Short delay after asserting ATN
        self.bus.delay_us(DELAY_DATA_US).await;

        log::debug!("ATN asserted, waiting for drive to acknowledge with DATA low...");

        // Drive must assert DATA within 1ms to acknowledge ATN
        self.bus.wait_for_data(true, DELAY_ACK_TIMEOUT_US).await?;

        // We are now in data send state. CLK should remain released (false) 
        // until send() is called, which will then pull it low to start the byte.
        self.bus.set_clk(false);
        self.bus.delay_us(DELAY_DATA_US).await;

        Ok(self)
    }

    /// Releases ATN and waits a short turnaround delay.
    pub async fn atn_release(self) -> Result<Self, IecError> {
        self.bus.trace_state("atn_release:start");

        // Wait the release timeout before ATN release
        self.bus.delay_us(DELAY_RELEASE_US).await;
        self.bus.set_atn(false);
        
        log::debug!("ATN released, waiting for drive to release DATA...");
        // The drive MUST release DATA as an acknowledgment of ATN release.
        self.bus.wait_for_data(false, DELAY_ACK_TIMEOUT_US).await?;

        self.bus.delay_us(DELAY_RELEASE_US).await;
        log::debug!("ATN release complete");
        self.bus.trace_state("atn_release:done");
        Ok(self)
    }

    // Sends a byte with EOI by waiting for the drive to ACK EOI before sending
    pub async fn send_eoi(self, byte: u8) -> Result<Self, IecError> {
        log::trace!("Preparing to send byte 0x{:02X} with EOI", byte);

        // Release the clock line to indicate we are ready to send and wait for ACK
        self.bus.set_clk(false);
        self.bus.wait_for_data(false, DELAY_ACK_TIMEOUT_US).await?;

        // Then simply wait for drive to ACK EOI
        self.bus.wait_for_data(true, DELAY_ACK_TIMEOUT_US).await?;

        // Then send the byte as normal
        self.send(byte).await
    }

    /// Sends a single byte over the IEC bus.
    pub async fn send(self, byte: u8) -> Result<Self, IecError> {
        log::debug!("Ready to send byte 0x{:02X} over IEC, waiting for ACK.", byte);

        // Release CLK to allow drive to exit previous state
        self.bus.set_clk(false);

        // Wait for Listener to release DATA (Ready to Receive).
        // NOTE! Hold delay is infinite, but we wait max 5 seconds as the drive should never take more.
        self.bus.wait_for_data(false, DELAY_HOLD_TIMEOUT_US).await?;

        // Set CLK low to prepare for bit transmission
        self.bus.delay_us(DELAY_DATA_US).await;
        self.bus.set_clk(true);

        // 2. Send each bit of the byte
        for i in 0..8 {
            let bit = (byte >> i) & 1 == 1;

            log::trace!("Sending bit {}: {}", i, bit);

            // Set DATA bit: logical 1 = Released (false), logical 0 = Asserted (true)
            self.bus.set_data(!bit);
            self.bus.delay_us(DELAY_DATA_US).await;

            // Release CLK to signal bit is ready hold data for at least 60us
            self.bus.set_clk(false);
            self.bus.delay_us(DELAY_DATA_US).await;

            // Re-assert CLK to prepare for next bit
            self.bus.set_clk(true);
        }

        // Release DATA
        self.bus.set_data(false);

        // 4. Wait for Acknowledge (DATA Low from drive)
        self.bus.wait_for_data(true, DELAY_ACK_TIMEOUT_US).await?;

        // Release CLK to allow drive to proceed
        self.bus.set_clk(false);

        // Wait 100 us after ACK to ensure drive has time to process the byte
        log::debug!("Byte 0x{:02X} sent, received ACK from drive", byte);

        Ok(self)
    }

    /// Receives a single byte from the IEC bus, returning the byte and whether it was an EOI condition.
    pub async fn recv(self, byte: &mut u8, eoi: &mut bool) -> Result<Self, IecError> {
        let mut val = 0u8;

        log::trace!("Turnaround: Releasing bus to allow drive to send data...");
        self.bus.trace_state("recv:turnaround");

        // Turnaround: If we were the talker, we must release the clock line, assert data line.
        self.bus.set_clk(false);
        self.bus.set_data(true);

        // Wait for the drive to assert CLK, indicating it's ready to send bits
        // NOTE! If the turnaround was already done, the host clock is already released and drive
        //       CLK is already asserted, so this will just pass through.
        self.bus.wait_for_clk(true, DELAY_ACK_TIMEOUT_US).await?;
        self.bus.trace_state("recv:ready");

        // Drive signaled ready to send, so we signal ready to receive.
        self.bus.wait_for_clk(false, DELAY_ACK_TIMEOUT_US).await?;
        self.bus.delay_us(DELAY_DATA_US).await;
        self.bus.set_data(false);

        self.bus.trace_state("recv:receiving");

        // Wait for clock for 200 us. If we timeout we have an EOI condition.
        let eoi_result = self.bus.wait_for_clk(true, DELAY_EOI_TIMEOUT_US).await;
        let is_eoi = if eoi_result.is_err() {
            log::trace!(
                "EOI detected: no clock from drive after {}us",
                DELAY_EOI_TIMEOUT_US
            );
            // Assert data to ACK the EOI, wait for once phase, and release the data line again.
            self.bus.set_data(true);
            self.bus.delay_us(DELAY_DATA_US).await;
            self.bus.set_data(false);

            // Finally, wait for the clock to be asserted again, which means the drive will send the EOI byte next.
            self.bus.wait_for_clk(true, DELAY_ACK_TIMEOUT_US).await?;
            log::trace!("EOI handshake complete");
            true
        } else {
            false
        };

        // Receive each bit in the byte
        for i in 0..8 {
            // Wait for Talker to release clock, which means a bit is ready
            self.bus.wait_for_clk(false, DELAY_ACK_TIMEOUT_US).await?;

            // Read DATA: Released (false) = 1, Asserted (true) = 0
            let bit = !self.bus.get_data();
            log::trace!("Received bit {}: {}", i, bit);
            val |= (bit as u8) << i;

            self.bus.wait_for_clk(true, DELAY_ACK_TIMEOUT_US).await?;
        }

        // Capture the received byte, and set the EOI flag if it was an EOI condition
        *byte = val;
        *eoi = is_eoi;

        log::debug!("Received byte 0x{:02X} [{}] from drive. EOI: {}", val, Self::safe_byte_to_char(val), is_eoi);

        // Acknowledge byte (Listener pulls DATA low)
        self.bus.set_data(true);

        // If this is EOI, we need to wait until the drive releases the CLK line. Then we release the data line.
        if is_eoi {
            self.bus.wait_for_clk(false, DELAY_ACK_TIMEOUT_US).await?;
            self.bus.set_data(false);
        }

        Ok(self)
    }


    /// Called to complete the communication. Releases all lines, and waits a short delay to ensure the drive has time to settle.
    pub async fn done(self) -> Result<(), IecError> {
        // Wait for once data phase, to ensure any ACKs are received
        self.bus.delay_us(DELAY_DATA_US).await;

        // Then release all lines to finalize
        self.bus.set_data(false);
        self.bus.set_clk(false);
        self.bus.set_atn(false);

        // Finally, wait the EOI delay for the drive to settle, before returning.
        self.bus.delay_us(DELAY_EOI_TIMEOUT_US).await;
        log::debug!("Communication finalized, bus released");

        Ok(())
    }

    // Helper to convert byte to a safe ASCII char for logging
    fn safe_byte_to_char(byte: u8) -> char {
        if byte.is_ascii_graphic() || byte == b' ' {
            byte as char
        } else {
            '.'
        }
    }

}

/// High-level IEC bus commander providing a more ergonomic API.
pub struct IecCommander<'a, B: IecComHostBus> {
    bus: &'a mut B,
    device: u8,
    secondary: u8,
    send_data: Option<&'a [u8]>,
}

impl<'a, B: IecComHostBus> IecCommander<'a, B> {
    pub fn new(bus: &'a mut B) -> Self {
        Self {
            bus,
            device: 8,
            secondary: 15,
            send_data: None,
        }
    }

    /// Sets the target device ID and secondary address.
    pub fn open(mut self, device: u8, secondary: u8) -> Self {
        self.device = device;
        self.secondary = secondary;
        self
    }

    /// Stages data to be sent to the drive.
    pub fn send(mut self, data: &'a [u8]) -> Self {
        self.send_data = Some(data);
        self
    }

    /// Executes the communication, receiving data into the provided callback until EOI.
    pub async fn receive<F>(self, mut f: F) -> Result<(), IecError>
    where
        F: FnMut(u8),
    {
        let mut com = IecTransaction::new(self.bus);

        // 1. If we have data to send, perform a LISTEN transaction first
        if let Some(data) = self.send_data {
            com = com
                .atn_assert()
                .await?
                .send(IEC_LISTEN | self.device)
                .await?
                .send(0x60 | self.secondary)
                .await?
                .atn_release()
                .await?;

            // Send data bytes, and use send_eoi for the last byte to properly signal EOI to the drive.
            if let Some((last, head)) = data.split_last() {
                for &b in head {
                    com = com.send(b).await?;
                }
                com = com.send_eoi(*last).await?;
            }


            com = com
                .atn_assert()
                .await?
                .send(IEC_UNLISTEN)
                .await?
                .atn_release()
                .await?;
        }

        // 2. Perform a TALK transaction to receive data
        com = com
            .atn_assert()
            .await?
            .send(IEC_TALK | self.device)
            .await?
            .send(0x60 | self.secondary)
            .await?
            .atn_release()
            .await?;

        loop {
            let mut byte = 0;
            let mut eoi = false;
            com = com.recv(&mut byte, &mut eoi).await?;
            f(byte);
            if eoi {
                break;
            }
        }

        // Finalize TALK with UNTALK
        com = com
            .atn_assert()
            .await?
            .send(IEC_UNTALK)
            .await?
            .atn_release()
            .await?;

        com.done().await
    }

    /// Executes the communication, performing only the LISTEN transaction if send data is staged.
    pub async fn execute(self) -> Result<(), IecError> {
        let mut com = IecTransaction::new(self.bus);

        if let Some(data) = self.send_data {
            com = com
                .atn_assert()
                .await?
                .send(IEC_LISTEN | self.device)
                .await?
                .send(0x60 | self.secondary)
                .await?
                .atn_release()
                .await?;

            // Send data bytes, and use send_eoi for the last byte to properly signal EOI to the drive.
            if let Some((last, head)) = data.split_last() {
                for &b in head {
                    com = com.send(b).await?;
                }
                com = com.send_eoi(*last).await?;
            }

            com = com
                .atn_assert()
                .await?
                .send(IEC_UNLISTEN)
                .await?
                .atn_release()
                .await?;
        }

        com.done().await
    }
}
