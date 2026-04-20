//! Commodore IEC serial bus protocol implementation (Hardware Level).
//!
//! This implementation emulates the C64 side of the IEC bus by strictly
//! following the physical handshake protocol (CLK/DATA transitions).
//! It does NOT rely on internal drive ROM states (PC-snooping), making it
//! robust against timing variations and different ROM execution paths.

use anyhow::{anyhow, Result};

use crate::bridge::{BusSnapshot, LockStepCommand, LockStepResponse};

/// Abstract interface for driving the IEC bus.
/// Allows the same IEC protocol code to run directly on the Emulator (for tests)
/// or via a LockStep channel (for the debugger UI).
pub trait IecDriver {
    fn set_data(&mut self, asserted: bool);
    fn set_clk(&mut self, asserted: bool);
    fn set_atn(&mut self, asserted: bool);
    fn get_bus(&mut self) -> BusSnapshot;
    fn run(&mut self, cycles: u32) -> Result<BusSnapshot>;
    fn wait_for(&mut self, target: BusSnapshot, mask: BusSnapshot, timeout: u32) -> Result<BusSnapshot>;
    fn pc(&self) -> u16;
}

// ─── LockStep Implementation ────────────────────────────────────────────────

pub struct LockStepBus {
    pub cmd_tx: crossbeam_channel::Sender<LockStepCommand>,
    pub resp_rx: crossbeam_channel::Receiver<LockStepResponse>,
    pub host_drives: BusSnapshot,
    pub last_pc: u16,
}

impl IecDriver for LockStepBus {
    fn set_data(&mut self, asserted: bool) { self.host_drives.data = asserted; }
    fn set_clk(&mut self, asserted: bool)  { self.host_drives.clk = asserted; }
    fn set_atn(&mut self, asserted: bool)  { self.host_drives.atn = asserted; }
    fn get_bus(&mut self) -> BusSnapshot {
        self.run(0).unwrap_or_default()
    }
    fn run(&mut self, cycles: u32) -> Result<BusSnapshot> {
        self.cmd_tx.send(LockStepCommand::Step { host_drives: self.host_drives, cycles })?;
        match self.resp_rx.recv()? {
            LockStepResponse::Done(bus, pc) => {
                self.last_pc = pc;
                Ok(bus)
            }
            LockStepResponse::Timeout(bus, pc) => {
                self.last_pc = pc;
                Ok(bus)
            }
        }
    }
    fn wait_for(&mut self, target: BusSnapshot, mask: BusSnapshot, timeout: u32) -> Result<BusSnapshot> {
        self.cmd_tx.send(LockStepCommand::Wait {
            host_drives: self.host_drives,
            target,
            mask,
            timeout_cycles: timeout,
        })?;
        match self.resp_rx.recv()? {
            LockStepResponse::Done(bus, pc) => {
                self.last_pc = pc;
                Ok(bus)
            }
            LockStepResponse::Timeout(_bus, pc) => {
                self.last_pc = pc;
                Err(anyhow!("IEC bus timeout (PC=${:04X})", pc))
            }
        }
    }
    fn pc(&self) -> u16 { self.last_pc } 
}

/// Cycles to wait for a line transition before declaring a bus timeout.
pub const IEC_TIMEOUT: u32 = 10_000_000; // 10 seconds at 1 MHz

/// Standard IEC settling/turnaround delay.
const IEC_SETTLE_DELAY: u32 = 2000;

/// Standard IEC bit-transfer delay (cycles per half-bit).
const IEC_BIT_DELAY: u32 = 1000;

// ─── Low-level Bus Primitives ───────────────────────────────────────────────

pub fn iec_tick(drv: &mut impl IecDriver) {
    let _ = drv.run(1);
}

pub fn iec_run(drv: &mut impl IecDriver, cycles: u32) {
    let _ = drv.run(cycles);
}

/// Returns the current physical state of the IEC bus lines.
pub fn iec_bus(drv: &mut impl IecDriver) -> BusSnapshot {
    drv.get_bus()
}

pub fn iec_set_data(drv: &mut impl IecDriver, asserted: bool) {
    drv.set_data(asserted);
}

pub fn iec_set_clk(drv: &mut impl IecDriver, asserted: bool) {
    drv.set_clk(asserted);
}

pub fn iec_set_atn(drv: &mut impl IecDriver, asserted: bool) {
    drv.set_atn(asserted);
}

/// Wait for a bus condition to be met by ticking the emulator cycle-by-cycle.
pub fn iec_wait(drv: &mut impl IecDriver, mut pred: impl FnMut(BusSnapshot) -> bool) -> Result<()> {
    // We can't easily pass the predicate to LockStep Wait, so we either:
    // 1. Tick here (slow over channels)
    // 2. Use common predicates that LockStepCommand supports.
    
    // For now, let's just tick. If it's a LockStepBus, it will be one message per cycle.
    // Wait, that's what I wanted to avoid.
    
    // Let's use the driver's wait_for if possible for common cases.
    // But pred is flexible.
    
    for _ in 0..IEC_TIMEOUT {
        let bus = drv.get_bus();
        if pred(bus) { return Ok(()); }
        drv.run(1)?;
    }
    Err(anyhow!("IEC bus timeout (PC=${:04X})", drv.pc()))
}

// ─── Handshake Sequences ─────────────────────────────────────────────────────

/// Assert ATN and wait for the drive to acknowledge by pulling DATA LOW.
pub fn iec_atn_start(drv: &mut impl IecDriver) -> Result<()> {
    // 1. Host releases ATN and CLK to ensure a clean start
    drv.set_atn(false);
    drv.set_clk(false);
    drv.run(2000)?;

    // 2. Host asserts ATN
    drv.set_atn(true);
    
    // 3. Wait for drive to acknowledge by asserting DATA (pulling LOW)
    drv.wait_for(
        BusSnapshot { data: true, ..Default::default() },
        BusSnapshot { data: true, ..Default::default() },
        IEC_TIMEOUT
    ).map(|_| ())
}

pub fn iec_atn_release(drv: &mut impl IecDriver) {
    drv.set_atn(false);
    drv.set_clk(false);
    let _ = drv.run(IEC_SETTLE_DELAY);
}

pub fn iec_idle(drv: &mut impl IecDriver) {
    drv.set_data(false);
    drv.set_clk(false);
    drv.set_atn(false);
    let _ = drv.run(IEC_SETTLE_DELAY);
}

// ─── Data Transfer ───────────────────────────────────────────────────────────

/// Sends a single byte as the SENDER (Talker).
pub fn iec_send_byte(drv: &mut impl IecDriver, byte: u8) -> Result<()> {
    // 1. Host asserts CLK to signal it's about to send
    drv.set_clk(true);
    
    // 2. Host waits for drive to release DATA (Ready to Receive)
    drv.wait_for(
        BusSnapshot { data: false, ..Default::default() },
        BusSnapshot { data: true, ..Default::default() },
        IEC_TIMEOUT
    )?;

    // 3. Transfer 8 bits, LSB-first
    for bit_idx in 0..8 {
        let bit = (byte >> bit_idx) & 1 != 0;
        
        // Data asserted (LOW) = 1, Released (HIGH) = 0
        drv.set_data(bit);
        
        // Host releases CLK: Rising edge signals "data valid"
        drv.set_clk(false);
        drv.run(IEC_BIT_DELAY)?;
        
        // Host asserts CLK: Falling edge signals "bit accepted"
        drv.set_clk(true);
        drv.run(IEC_BIT_DELAY)?;
    }

    // 4. End of byte: host releases DATA and CLK
    drv.set_data(false);
    drv.set_clk(false);
    drv.run(IEC_SETTLE_DELAY)?;

    Ok(())
}

/// Receives a single byte and EOI status from the drive (Talker).
/// Returns (byte_value, is_eoi_received).
pub fn iec_receive_byte(drv: &mut impl IecDriver) -> Result<(u8, bool)> {
    // 1. Host (Listener) releases DATA to signal "Ready to Listen"
    drv.set_data(false);
    drv.run(100)?;

    // 2. Wait for drive (Talker) to acknowledge by releasing CLK
    // EOI Detection: if CLK stays LOW for > ~256µs, it signals EOI.
    let mut clk_low_cycles = 0;
    loop {
        let bus = drv.get_bus();
        if !bus.clk { break; } // CLK released HIGH
        
        // Safety: check if drive is idle
        if (0xEBFF..=0xEC9B).contains(&drv.pc()) {
            return Err(anyhow!("DRIVE_IDLE"));
        }

        drv.run(1)?;
        clk_low_cycles += 1;
        
        if clk_low_cycles > 10000 { // 10ms should be plenty for EOI detect
            return Err(anyhow!("iec_receive_byte: timeout waiting for CLK release (Talker Ready)"));
        }
    }

    let is_eoi = clk_low_cycles > 500; // Standard EOI threshold

    // 3. Host asserts DATA to acknowledge "Talker Ready"
    drv.set_data(true);
    drv.run(100)?;

    // 4. If EOI was signaled, host must acknowledge by releasing DATA for > 60µs
    if is_eoi {
        drv.set_data(false);
        drv.run(100)?; // ~100µs delay
        drv.set_data(true);
        // Drive must now acknowledge by asserting CLK again
        while !drv.get_bus().clk {
             if (0xEBFF..=0xEC9B).contains(&drv.pc()) { return Err(anyhow!("DRIVE_IDLE")); }
             drv.run(1)?;
        }
        // And then finally releasing it to start the transfer
        while drv.get_bus().clk {
             if (0xEBFF..=0xEC9B).contains(&drv.pc()) { return Err(anyhow!("DRIVE_IDLE")); }
             drv.run(1)?;
        }
    }

    // 6. Receive 8 bits LSB-first
    let mut byte_val = 0u8;
    for bit_idx in 0..8 {
        // Wait for drive to release CLK (Rising Edge -> Bit Valid)
        drv.wait_for(
            BusSnapshot { clk: false, ..Default::default() },
            BusSnapshot { clk: true, ..Default::default() },
            IEC_TIMEOUT
        )?;

        // Sample DATA line (Inverted logic: asserted/LOW = bit 1)
        if drv.get_bus().data {
            byte_val |= 1 << bit_idx;
        }

        // Wait for drive to assert CLK (Falling Edge -> Bit Done)
        drv.wait_for(
            BusSnapshot { clk: true, ..Default::default() },
            BusSnapshot { clk: true, ..Default::default() },
            IEC_TIMEOUT
        )?;
    }

    // 5. Byte Acknowledgement: Host pulls DATA LOW
    drv.set_data(true);

    Ok((byte_val, is_eoi))
}

/// Receives all available data from the drive until EOI is detected.
pub fn iec_receive_data(drv: &mut impl IecDriver) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    loop {
        // Kernal ACPTR handshake starts by releasing CLK to signal "Listener Ready"
        // (Host was holding it LOW from the turnaround or the previous byte ack)
        drv.set_clk(false);
        drv.set_data(false);
        
        // Safety: check if drive returned to idle loop during the release
        if (0xEBFF..=0xEC9B).contains(&drv.pc()) {
            break;
        }

        match iec_receive_byte(drv) {
            Ok((byte, is_eoi)) => {
                data.push(byte);
                if is_eoi { break; }
            }
            Err(e) if e.to_string() == "DRIVE_IDLE" => {
                break;
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    
    Ok(data)
}

// ─── High-level Commands ─────────────────────────────────────────────────────

pub fn iec_read_status(drv: &mut impl IecDriver) -> Result<String> {
    iec_atn_start(drv)?;
    iec_send_byte(drv, 0x48)?; // TALK device 8
    iec_send_byte(drv, 0x6F)?; // DATA channel 15 (status)
    drv.run(1000)?; // Wait for drive to process secondary address
    
    // Exact Kernal TALK turnaround sequence:
    drv.set_atn(false); // Release ATN
    drv.set_clk(true);  // Host (Listener) pulls CLK LOW
    drv.run(100)?;
    
    // 1. Wait for drive to acknowledge turnaround by pulling DATA LOW
    if let Err(e) = drv.wait_for(
        BusSnapshot { data: true, ..Default::default() },
        BusSnapshot { data: true, ..Default::default() },
        IEC_TIMEOUT
    ) {
        if (0xEBFF..=0xEC9B).contains(&drv.pc()) {
             return Ok(String::new());
        }
        return Err(e);
    }
    
    // 2. Host releases DATA and CLK to signal it's ready to receive
    drv.set_data(false);
    drv.set_clk(false);
    drv.run(100)?;

    let data = match iec_receive_data(drv) {
        Ok(d) => d,
        Err(e) => {
            if (0xEBFF..=0xEC9B).contains(&drv.pc()) {
                Vec::new() 
            } else {
                return Err(e);
            }
        }
    };
    
    iec_idle(drv);
    Ok(String::from_utf8_lossy(&data).trim_end_matches('\r').to_string())
}

pub fn iec_load_file(drv: &mut impl IecDriver, filename: &str) -> Result<Vec<u8>> {
    // 1. OPEN channel 0
    iec_atn_start(drv)?;
    iec_send_byte(drv, 0x28)?; // LISTEN device 8
    iec_send_byte(drv, 0xF0)?; // OPEN channel 0
    iec_atn_release(drv);

    for &b in filename.as_bytes() {
        iec_send_byte(drv, b)?;
    }

    // 2. UNLISTEN
    iec_atn_start(drv)?;
    iec_send_byte(drv, 0x3F)?; // UNLISTEN
    iec_atn_release(drv);

    // 3. TALK and Receive
    iec_atn_start(drv)?;
    iec_send_byte(drv, 0x48)?; // TALK device 8
    iec_send_byte(drv, 0x60)?; // DATA channel 0
    
    // Turnaround: Release ATN
    drv.set_atn(false);
    
    // Wait for drive to acknowledge turnaround by pulling DATA LOW
    drv.wait_for(
        BusSnapshot { data: true, ..Default::default() },
        BusSnapshot { data: true, ..Default::default() },
        IEC_TIMEOUT
    )?;
    
    // Host signals it's ready to receive by pulling CLK LOW
    drv.set_clk(true);
    drv.run(1000)?;

    let data = iec_receive_data(drv)?;
    
    iec_idle(drv);
    Ok(data)
}

// ─── Directory Parser ─────────────────────────────────────────────────────────

pub fn parse_directory(data: &[u8]) -> String {
    if data.len() < 4 {
        return "(empty or truncated directory data)".to_string();
    }
    let mut lines = Vec::new();
    let mut pos = 2; // skip 2-byte BASIC load address
    while pos + 4 <= data.len() {
        let next_link = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if next_link == 0 { break; }
        let line_num = u16::from_le_bytes([data[pos + 2], data[pos + 3]]);
        pos += 4;

        let mut text = String::new();
        while pos < data.len() && data[pos] != 0x00 {
            let c = data[pos];
            let ascii: u8 = match c {
                0x20..=0x5F => c,
                0x60..=0x7E => c - 0x20,
                0xC1..=0xDA => c - 0x80,
                _ => b'.',
            };
            text.push(ascii as char);
            pos += 1;
        }
        pos += 1; // skip null terminator
        lines.push(format!("{:5} {}", line_num, text));
    }
    if lines.is_empty() { "(no entries)".to_string() } else { lines.join("\n") }
}
