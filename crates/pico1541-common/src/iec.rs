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

/// Hardware abstraction for the IEC bus.
/// 
/// All signals use the logical convention: **true = asserted**.
/// On the physical IEC bus an asserted line is pulled LOW.
#[allow(async_fn_in_trait)]
pub trait IecBus {
    fn set_atn(&mut self, assert: bool);
    fn set_clk(&mut self, assert: bool);
    fn set_data(&mut self, assert: bool);
    fn get_clk(&self) -> bool;
    fn get_data(&self) -> bool;

    async fn delay_us(&self, us: u32);
    
    async fn wait_for_clk(&self, assert: bool, timeout_us: u32) -> Result<(), IecError> {
        let mut elapsed = 0;
        while self.get_clk() != assert {
            if elapsed >= timeout_us {
                return Err(IecError::Timeout);
            }
            self.delay_us(10).await;
            elapsed += 10;
        }
        Ok(())
    }

    async fn wait_for_data(&self, assert: bool, timeout_us: u32) -> Result<(), IecError> {
        let mut elapsed = 0;
        while self.get_data() != assert {
            if elapsed >= timeout_us {
                return Err(IecError::Timeout);
            }
            self.delay_us(10).await;
            elapsed += 10;
        }
        Ok(())
    }
}

/// Universal IEC protocol tool acting as a Host (e.g. C64).
pub struct IecBusCom<B: IecBus> {
    bus: B,
}

impl<B: IecBus> IecBusCom<B> {
    pub fn new(bus: B) -> Self {
        Self { bus }
    }

    /// Pulls ATN low and waits for the drive to acknowledge by pulling DATA low.
    pub async fn atn_assert(mut self) -> Result<Self, IecError> {
        self.bus.set_atn(true);
        // Drive must pull DATA low within 1ms to acknowledge ATN
        self.bus.wait_for_data(true, 1000).await?;
        
        // Host then pulls CLK low and releases DATA
        self.bus.set_clk(true);
        self.bus.set_data(false);
        self.bus.delay_us(20).await;
        
        Ok(self)
    }

    /// Releases ATN and waits a short turnaround delay.
    pub async fn atn_release(mut self) -> Result<Self, IecError> {
        self.bus.set_atn(false);
        self.bus.delay_us(20).await;
        Ok(self)
    }

    /// Asserts CLK (Host master).
    pub async fn clk_assert(mut self) -> Result<Self, IecError> {
        self.bus.set_clk(true);
        Ok(self)
    }

    /// Releases CLK.
    pub async fn clk_release(mut self) -> Result<Self, IecError> {
        self.bus.set_clk(false);
        Ok(self)
    }

    /// Asserts DATA.
    pub async fn data_assert(mut self) -> Result<Self, IecError> {
        self.bus.set_data(true);
        Ok(self)
    }

    /// Releases DATA.
    pub async fn data_release(mut self) -> Result<Self, IecError> {
        self.bus.set_data(false);
        Ok(self)
    }

    /// Sends a single byte over the IEC bus.
    pub async fn send(mut self, byte: u8) -> Result<Self, IecError> {
        // 1. Wait for Listener to release DATA (Ready to Receive)
        self.bus.wait_for_data(false, 1000).await?;
        
        for i in 0..8 {
            let bit = (byte >> i) & 1 == 1;
            
            // Set DATA bit (true = asserted = physical Low = 1)
            self.bus.set_data(bit);
            self.bus.delay_us(20).await;
            
            // Pull CLK Low (strobe)
            self.bus.set_clk(true);
            self.bus.delay_us(40).await;
            
            // Release CLK
            self.bus.set_clk(false);
            self.bus.delay_us(40).await;
        }
        
        // 3. Release DATA
        self.bus.set_data(false);
        
        // 4. Wait for Acknowledge (DATA Low from drive)
        self.bus.wait_for_data(true, 1000).await?;
        
        Ok(self)
    }

    /// Receives a single byte from the IEC bus.
    pub async fn recv(mut self, byte: &mut u8) -> Result<Self, IecError> {
        let mut val = 0u8;
        
        // 1. Host (Listener) releases DATA to signal it's ready to receive
        self.bus.set_data(false);
        
        for i in 0..8 {
            // Wait for Talker to pull CLK low (Strobe)
            self.bus.wait_for_clk(true, 1000).await?;
            
            // Read DATA
            if self.bus.get_data() {
                val |= 1 << i;
            }
            
            // Wait for Talker to release CLK
            self.bus.wait_for_clk(false, 1000).await?;
        }
        
        *byte = val;
        
        // 3. Acknowledge byte (Listener pulls DATA low)
        self.bus.set_data(true);
        self.bus.delay_us(40).await;
        
        Ok(self)
    }

    /// Ends the communication and returns the underlying bus.
    pub fn done(self) -> B {
        self.bus
    }
}
