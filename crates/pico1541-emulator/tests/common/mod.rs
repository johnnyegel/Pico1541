#![allow(dead_code)]
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;

use log;

use racc6502::cpu::DebugHook;

//const ROM_BYTES: &[u8] = include_bytes!("roms/1541-II.251968-03.bin");
const ROM_BYTES: &[u8] = include_bytes!("../../../../roms/1541-II.251968-03.bin");

/// Timeout for spin-waiting on bus transitions (cycles).
/// This can take a while in some cases, so we set this to 20 seconds for now
pub const IEC_TIMEOUT: u32 = 20_000_000;

/// Standard IEC Timings (in CPU cycles @ 1MHz)
pub const T_SETTLE: u32 = 100;  // ~100us (settle time)
pub const T_PHASE:  u32 = 60;   // ~60us (C64 minimum rate)
pub const T_EOI:    u32 = 200;  // >200us

/// 1541-II ROM idle loop address range.
pub const IDLE_LOW:  u16 = 0xEBFF;
pub const IDLE_HIGH: u16 = 0xEC9B;

pub fn setup_drive(reset: bool, debug_hook: Option<DebugHook>) -> (Emulator<TestBridge>, TestBridge) {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();

    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);

    let mut drive = Emulator::<TestBridge>::new(rom);
    let mut bridge = TestBridge::new();

    if let Some(hook) = debug_hook {
        drive.cpu.debug_hook = Some(hook);
    }

    if reset {
        reset_drive(&mut drive, &mut bridge);
    }

    (drive, bridge)
}

pub fn reset_drive(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) {
    drive.reset();
    bridge.reset();

    // Boot: Wait for idle loop
    for _ in 0..5_000_000 {
        drive.tick(bridge);
        if drive.pc() >= IDLE_LOW && drive.pc() <= IDLE_HIGH {
            return;
        }
    }

    panic!("Drive did not reach idle loop after reset (PC=${:04X})", drive.pc());
}


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
        log::trace!("Ticked {} cycles (PC=${:04X})", n, drive.pc());
    }

    pub fn wait_bus_data(&mut self, drive: &mut Emulator<Self>, want: bool) { 
        let mut cycles = 0;
        if self.bus_data == want { return; }
        for _ in 0..IEC_TIMEOUT {
            cycles += 1;
            if self.bus_data == want { 
                log::trace!("Wait Data {} took {} cycles. PC=${:04X}", want, cycles, drive.pc());
                return; 
            }
            drive.tick(self);
        }
        panic!("IEC timeout: DATA != {} (PC=${:04X}, host_data={})", want, drive.pc(), self.host_data);
    }

    pub fn wait_bus_clk(&mut self, drive: &mut Emulator<Self>, want: bool) {
        let mut cycles = 0;
        if self.bus_clk == want { return; }
        for _ in 0..IEC_TIMEOUT {
            cycles += 1;
            if self.bus_clk == want { 
                log::trace!("Wait CLK {} took {} cycles. PC=${:04X}", want, cycles, drive.pc());
                return; 
            }
            drive.tick(self);
        }
        panic!("IEC timeout: CLK != {} (PC=${:04X}, host_clk={})", want, drive.pc(), self.host_clk);
    }

    /// Signal start of an IEC command sequence by asserting CLK, and waiting for the drive to ACK
    pub fn atn_start(&mut self, drive: &mut Emulator<Self>) {
        // Host asserts ATN to signal start of command sequence
        self.set_atn(true);

        // Wait one clock phase, and assert the clock
        self.ticks(drive, T_PHASE);
        self.set_clk(true);

        log::trace!("ATN start: Host asserted ATN. Waiting for drive to acknowledge... PC=${:04X}", drive.pc());

        // Drive must acknowledge by asserting DATA
        self.wait_bus_data(drive, true);

        log::trace!("ATN acknowledged: Drive asserted DATA. PC=${:04X}", drive.pc());
    }

    pub fn atn_end(&mut self, drive: &mut Emulator<Self>) {
        // Wait one clock phase before releasing the ATN
        self.ticks(drive, T_PHASE);

        // Host releases ATN to signal end of command sequence. There is no ACK for this!
        self.set_atn(false);

        log::debug!("ATN end: Host released ATN... PC=${:04X}", drive.pc());
    }

    // Called a the end of an IEC command sequence to release all lines 
    // and wait for the drive to release the data line if it was asserted.
    pub fn iec_end(&mut self, drive: &mut Emulator<Self>) {
        // Wait one phase
        self.ticks(drive, T_PHASE);

        // Release all lines
        self.set_atn(false);
        self.set_clk(false);
        self.set_data(false);

        // Wait for the data line to be released (if asserted).
        if self.bus_data {
            self.wait_bus_data(drive, false);
        }

        // Wait one phase to settle
        self.ticks(drive, T_PHASE);
    }

    pub fn send_byte(&mut self, drive: &mut Emulator<Self>, byte: u8, eoi: bool) { 
        // Clock is Asserted before we enter this function!
        if !self.host_clk {
            panic!("send_byte called with CLK not asserted! PC=${:04X}", drive.pc());
        }

        log::trace!("Sending byte 0x{:02X} with EOI={}... PC=${:04X}", byte, eoi, drive.pc());

        // Release the clock to signal ready to send, and wait for the drive to ACK by asserting DATA.
        self.set_clk(false);
        self.wait_bus_data(drive, false);

        log::debug!("Drive ACKed: Sending byte 0x{:02X}. EOI: {}", byte, eoi);

        // Is this end of instruction?
        if eoi {
            // To signal EOI, we wait for the EOI delay, 
            // and then assert the data being asserted and deassered 
            // again to indicate the drive has gotten the message.
            log::trace!("Waiting for ACK for EOI for byte 0x{:02X}. PC=${:04X}", byte, drive.pc());

            self.wait_bus_data(drive, true);
            self.wait_bus_data(drive, false);

            log::trace!("EOI acknowledged for byte 0x{:02X}", byte);
        }

        // Wait one intial clock phase before starting. Then assert clock to initial state
        self.ticks(drive, T_PHASE);
        self.set_clk(true);

        // Iterate over bits.    
        for bit in 0..8u8 {
            // Get the next bit to send (1=Asserted/LOW wire)
            let bit_val = (byte >> bit) & 1 != 0;
            
            // Wait for half a clock phase, set data and wait for the rest of the phase to elapse (to meet setup/hold timing).
            // This ensures data is set in the center of the clock high period, which is more robust for the drive to sample reliably.
            self.set_data(!bit_val);
            self.ticks(drive, T_PHASE);

            // Release the clock line to signal the bit is ready to be read by the drive.
            // Hold this state for a full clock phase to meet the drive's timing requirements.
            self.set_clk(false);
            self.ticks(drive, T_PHASE);

            // Assert the clock at the end of the bit
            self.set_clk(true);

            log::trace!("Sent 0x{:02X} bit {} ({}), PC=${:04X}", 
                    byte, bit, if bit_val {1} else {0}, drive.pc());
        }

        // Release the data line (if last bit asserted the line)
        self.set_data(false);

        // Wait for the drive to assert the data line to acknowledge the byte
        log::trace!("Drive Data Line ACK: {}", self.bus_data);
        self.wait_bus_data(drive, true);
    }



    pub fn turnaround(&mut self, drive: &mut Emulator<Self>) {
        // Host releases ATN and CLK to signal it's now a listener
        self.set_clk(false);

        // Host asserts DATA to indicate it is busy and not signaling EOI (TRUE=Asserted=Not EOI)
        self.set_data(true);

        // Drive must release DATA (ACK for the last byte of command) before it can become a Talker
        self.wait_bus_data(drive, false);

        // Drive must acknowledge ATN release by asserting CLK
        self.wait_bus_clk(drive, true);
    }

    pub fn receive_byte(&mut self, drive: &mut Emulator<Self>) -> (u8, bool) {
        // 1. Wait for Talker Ready (CLK released)
        // Host should already be asserting DATA (TRUE) from turnaround or previous byte.
        self.wait_bus_clk(drive, false);
        
        // 2. Host RELEASES DATA to acknowledge Talker Ready
        self.set_data(false);

        // 3. Wait a bit to see if drive asserts CLK (Normal Byte)
        // In the normal path, the drive asserts CLK as soon as it sees DATA released.
        // In the EOI path, it will continue to wait for DATA to be re-asserted.
        let mut ack_ticks = 0;
        while !self.bus_clk && ack_ticks < 500 {
            drive.tick(self);
            ack_ticks += 1;
        }

        let is_eoi = if self.bus_clk {
            false
        } else {
            // Drive is still waiting. This is the EOI handshake (waiting for DATA re-assertion).
            self.set_data(true);
            self.wait_bus_clk(drive, true);
            // After drive acknowledges our EOI ACK, we release DATA for the bit loop
            self.set_data(false);
            true
        };

        // 4. Small delay before starting bits
        self.ticks(drive, T_PHASE);

        // 5. Receive 8 bits
        let mut byte_val = 0u8;
        for bit in 0..8u8 {
            self.wait_bus_clk(drive, false);
            if !self.bus_data {
                byte_val |= 1 << bit;
            }
            self.wait_bus_clk(drive, true);
        }

        // 6. After the byte, Host asserts DATA to say "I'm busy" 
        // and also signals "Not EOI" for the next potential byte.
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
        }
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
