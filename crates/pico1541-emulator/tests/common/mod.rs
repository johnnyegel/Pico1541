#![allow(dead_code)]
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;

/// Timeout for spin-waiting on bus transitions (cycles).
pub const IEC_TIMEOUT: u32 = 1_000_000;

/// Standard IEC Timings (in CPU cycles @ 1MHz)
pub const T_SETTLE: u32 = 100;  // ~100us (settle time)
pub const T_PHASE:  u32 = 60;   // ~60us (C64 minimum rate)
pub const T_EOI:    u32 = 200;  // >200us

// FIXME! Remove these as they are utter bullshit
pub const T_BIT:    u32 = 40;   // ~40us (full bit time, including clock low)

/// 1541-II ROM idle loop address range.
pub const IDLE_LOW:  u16 = 0xEBFF;
pub const IDLE_HIGH: u16 = 0xEC9B;

/// Shared test double for HostInterface.
pub struct TestBridge {
    pub track: ActiveTrack,
    pub events: Vec<DriveEvent>,
    pub host_data: bool,
    pub host_clk: bool,
    pub host_atn: bool,
    pub bus_data: bool,
    pub bus_clk: bool,
}

impl TestBridge {
    pub fn new() -> Self {
        Self {
            track: ActiveTrack::new(),
            events: Vec::new(),
            host_data: false,
            host_clk: false,
            host_atn: false,
            bus_data: false,
            bus_clk: false,
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.host_data = false;
        self.host_clk = false;
        self.host_atn = false;
        self.bus_data = false;
        self.bus_clk = false;
    }

    pub fn set_atn(&mut self, v: bool)  { self.host_atn = v; }
    pub fn set_clk(&mut self, v: bool)  { self.host_clk = v; }
    pub fn set_data(&mut self, v: bool) { self.host_data = v; }

    pub fn ticks(&mut self, drive: &mut Emulator<TestBridge>, n: u32) {
        for _ in 0..n { drive.tick(self); }
        println!("Ticked {} cycles (PC=${:04X})", n, drive.pc());
    }

    pub fn wait_bus_data(&mut self, drive: &mut Emulator<Self>, want: bool) { 
        let mut cycles = 0;
        if self.bus_data == want { 
            panic!("wait_bus_data called but bus_data is already {} (PC=${:04X})", want, drive.pc());
        }
        for _ in 0..IEC_TIMEOUT {
            cycles += 1;
            if self.bus_data == want { 
                println!("Wait Data {} took {} cycles. PC=${:04X}", want, cycles, drive.pc());
                return; 
            }
            drive.tick(self);
        }
        panic!("IEC timeout: DATA != {} (PC=${:04X}, host_data={})", want, drive.pc(), self.host_data);
    }

    pub fn wait_bus_clk(&mut self, drive: &mut Emulator<Self>, want: bool) {
        let mut cycles = 0;
        if self.bus_clk == want { 
            panic!("wait_bus_clk called but bus_clk is already {} (PC=${:04X})", want, drive.pc());
        }
        for _ in 0..IEC_TIMEOUT {
            cycles += 1;
            if self.bus_clk == want { 
                println!("Wait CLK {} took {} cycles. PC=${:04X}", want, cycles, drive.pc());
                return; 
            }
            drive.tick(self);
        }
        panic!("IEC timeout: CLK != {} (PC=${:04X}, host_clk={})", want, drive.pc(), self.host_clk);
    }

    pub fn atn_start(&mut self, drive: &mut Emulator<Self>) {
        // Host asserts ATN and asserts the clock to signal a command
        self.set_atn(true);
        self.set_clk(true);

        // Drive must acknowledge ATN by asserting DATA
        self.wait_bus_data(drive, true);
    }

    pub fn send_byte(&mut self, drive: &mut Emulator<Self>, byte: u8, eoi: bool) { 
        // Release CLK, and wait for drive to release DATA (ACK)
        self.set_clk(false);
        self.wait_bus_data(drive, false);

        println!("Drive ACKed: Ready to send byte {:02X}. EOI: {}", byte, eoi);

        // Is this end of instruction?
        if eoi {
            // To signal EOI, we wait for the EOI delay, 
            // and then assert the data being asserted and deassered 
            // again to indicate the drive has gotten the message.
            //self.ticks(drive, T_EOI);
            println!("Waiting for ACK for EOI for byte {:02X}. PC=${:04X}", byte, drive.pc());

            self.wait_bus_data(drive, true);
            self.wait_bus_data(drive, false);

            println!("EOI acknowledged for byte {:02X}", byte);
        }

        // We need to wait some more so the drive is ready to receive the first bit when clock is released.
        // The doc says this should not be needed, but clearly it is.
        self.ticks(drive, T_PHASE);


        // Pull the clock to true before sending the first byte
        self.set_clk(true);
        self.set_data(false);

        // Iterate over bits.    
        for bit in 0..8u8 {
            // Get the next bit to send (1=Asserted/LOW wire)
            let bit_val = (byte >> bit) & 1 != 0;

            // Wait for half a clock phase, set data and wait for the rest of the phase to elapse (to meet setup/hold timing).
            // This ensures data is set in the center of the clock high period, which is more robust for the drive to sample reliably.
            self.ticks(drive, T_PHASE / 2);
            self.set_data(!bit_val);
            self.ticks(drive, T_PHASE / 2);

            // Release the clock line to signal the bit is ready to be read by the drive.
            // Hold this state for a full clock phase to meet the drive's timing requirements.
            self.set_clk(false);
            self.ticks(drive, T_PHASE);
            self.set_clk(true);
            self.set_data(false);
            
            println!("Drive Data Line {}: -> {} - drive: {}", bit, bit_val, self.bus_data);
        }

        // Wait for the drive to assert the data line to acknowledge the byte
        println!("Drive Data Line ACK: {}", self.bus_data);

        self.wait_bus_data(drive, true);
    }

    pub fn receive_byte(&mut self, drive: &mut Emulator<Self>) -> (u8, bool) {
        let mut clk_high_cycles = 0;
        loop {
            if !self.bus_clk { break; }
            drive.tick(self);
            clk_high_cycles += 1;
            if clk_high_cycles > IEC_TIMEOUT { panic!("IEC timeout waiting for Talker Ready"); }
        }
        let is_eoi = clk_high_cycles > T_EOI;
        self.set_data(true);
        self.wait_bus_clk(drive, true);
        self.set_data(false);
        let mut byte_val = 0u8;
        for bit in 0..8u8 {
            self.wait_bus_clk(drive, false);
            if !self.bus_data { byte_val |= 1 << bit; }
            self.wait_bus_clk(drive, true);
        }
        self.set_data(true);
        self.ticks(drive, T_SETTLE);
        (byte_val, is_eoi)
    }

    pub fn receive_all(&mut self, drive: &mut Emulator<Self>) -> Vec<u8> {
        let mut data = Vec::new();
        loop {
            let (byte, is_eoi) = self.receive_byte(drive);
            data.push(byte);
            if is_eoi { break; }
            self.set_data(false);
            self.ticks(drive, T_BIT);
        }
        self.set_data(false);
        self.ticks(drive, T_SETTLE);
        data
    }

}

impl IecBus for TestBridge {
    fn read_data(&self) -> bool { self.host_data }
    fn read_clk(&self)  -> bool { self.host_clk }
    fn read_atn(&self)  -> bool { self.host_atn }
    fn drive_data(&mut self, v: bool) { self.bus_data = v; }
    fn drive_clk(&mut self, v: bool)  { self.bus_clk = v; }
}

impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, event: DriveEvent) { self.events.push(event); }
}
