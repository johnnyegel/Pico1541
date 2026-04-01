//! Commodore IEC serial bus protocol implementation.
//!
//! The host (debugger) emulates the C64 side of the IEC bus.
//! All IEC helpers run the emulator themselves cycle-by-cycle, so they must be
//! called with the emulator paused (the regular run loop must not be ticking
//! concurrently).
//!
//! Timing is deliberately conservative: each half-bit phase runs at least
//! IEC_BIT_DELAY cycles, giving the 1541 ROM's polling loops enough time to
//! execute multiple iterations before the signal changes again.
//!
//! IEC bus convention (compact IecBus format, open-drain asserted):
//!   bit 0 = DATA  (1=LOW/asserted, 0=HIGH/released)
//!   bit 1 = CLK   (1=LOW/asserted, 0=HIGH/released)
//!   bit 2 = ATN   (1=LOW/asserted, 0=HIGH/released)
//!
//! VIA1 inputs (7406 inversion captured by convention, direct mapping):
//!   PB0 = DATA_IN  (1 when DATA is asserted/LOW)
//!   PB2 = CLK_IN   (1 when CLK is asserted/LOW)
//!   PB7 = ATN_IN   (1 when ATN is asserted/LOW)

use anyhow::{anyhow, Result};
use pico1541_emulator::{interface::IecBus, Emulator};

use crate::bridge::HostBridge;

/// Cycles per half-bit phase. The 1541 ROM polling loop is ~10–15 cycles/iter;
/// 200 cycles gives it ~15+ iterations to sample each signal edge.
const IEC_BIT_DELAY: u32 = 1000;

/// Settling/turnaround delay used after ATN changes and talker handshakes.
const IEC_SETUP_DELAY: u32 = 2000;

/// Maximum cycles to wait for any bus event before declaring a timeout.
pub const IEC_TIMEOUT: u32 = 10_000_000; // 10 seconds at 1 MHz

// ─── Low-level helpers ────────────────────────────────────────────────────────

pub fn iec_tick(drive: &mut Emulator<HostBridge>) {
    drive.cpu.tick();
}

pub fn iec_run(drive: &mut Emulator<HostBridge>, n: u32) {
    for _ in 0..n { drive.cpu.tick(); }
}

/// Combined bus state as last written by the emulator's open-drain model.
#[inline]
pub fn iec_bus(drive: &Emulator<HostBridge>) -> IecBus {
    drive.cpu.bus.hardware.bus_state
}

pub fn iec_set_data(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.host_drives.set_data(asserted);
}
pub fn iec_set_clk(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.host_drives.set_clk(asserted);
}
pub fn iec_set_atn(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.host_drives.set_atn(asserted);
}

/// Run the emulator until `pred(bus_state)` returns true, or IEC_TIMEOUT cycles.
pub fn iec_wait_timeout(drive: &mut Emulator<HostBridge>, timeout: u32, pred: impl Fn(IecBus) -> bool) -> Result<()> {
    for _ in 0..timeout {
        iec_tick(drive);
        if pred(iec_bus(drive)) { return Ok(()); }
    }
    let pc = drive.cpu.pc;
    Err(anyhow!("IEC timeout (CPU PC = ${:04X})", pc))
}

pub fn iec_wait(drive: &mut Emulator<HostBridge>, pred: impl Fn(IecBus) -> bool) -> Result<()> {
    iec_wait_timeout(drive, IEC_TIMEOUT, pred)
}

// ─── Handshake ────────────────────────────────────────────────────────────────

/// Assert ATN and wait for the drive to acknowledge by pulling DATA LOW.
///
/// The 1541 ROM responds two ways:
///   • CA1 interrupt: fires on ATN's positive edge at the VIA (LOW→HIGH transition
///     after the 7406 inverter). `via1.last_ca1` must be `false` before we assert
///     ATN, otherwise the edge detector sees true→true and no IRQ fires. We
///     guarantee this by releasing ATN for a few cycles first.
///   • PB7 polling: the ROM's main loop also reads ATN_IN (PB7) directly.
///     This is the fallback if the interrupt was already missed.
pub fn iec_atn_start(drive: &mut Emulator<HostBridge>) -> Result<()> {
    // Release ATN and run a few cycles so CA1 goes LOW → last_ca1 = false
    iec_set_atn(drive, false);
    iec_set_clk(drive, true); // host asserts CLK throughout
    iec_run(drive, 20);       // just enough to register the LOW on CA1

    // Now assert ATN — 7406 inverts to HIGH at CA1 → positive edge → IRQ
    iec_set_atn(drive, true);
    iec_run(drive, IEC_SETUP_DELAY);
    iec_wait(drive, |b| b.data())
}

/// Release ATN and run a settling delay.
pub fn iec_atn_release(drive: &mut Emulator<HostBridge>) {
    iec_set_atn(drive, false);
    iec_run(drive, IEC_SETUP_DELAY);
}

/// Release the bus to idle (all lines HIGH/released).
pub fn iec_idle(drive: &mut Emulator<HostBridge>) {
    iec_set_data(drive, false);
    iec_set_clk(drive, false);
    iec_set_atn(drive, false);
}

// ─── Byte send (host is SENDER) ───────────────────────────────────────────────

/// Send one byte.  ATN may be asserted or released; the protocol is the same.
///
/// The 1541 ROM byte-receive routine (LE9C9/LEA0B) expects:
///   1. CLK released → ROM (LE87B/LE9CD) exits its CLK-asserted wait,
///      drive asserts own CLK (LE99C), waits for host DATA released via LFF20
///      (PB0 = host DATA only, so exits immediately), then waits at LE9DF for CLK asserted.
///   2. CLK asserted → LE9DF exits → ROM enters bit-read loop (LEA0B).
///   3. CLK released → bit N valid on DATA; CLK asserted → bit done (×8).
///      LEA0B checks PB2 = host CLK only (drive's own CLK_OUT does not feed back).
///   4. After bit 7: ROM releases drive CLK (LE9A5) and returns. No DATA handshake
///      during ATN-phase command bytes; host uses a timing delay instead.
pub fn iec_send_byte(drive: &mut Emulator<HostBridge>, byte: u8) -> Result<()> {
    // Step 1: release CLK → ROM (LE87B or LE9CD) exits, drive asserts CLK (LE99C),
    // LFF20 exits immediately (host DATA not asserted), ROM reaches LE9DF.
    iec_set_clk(drive, false);
    iec_run(drive, IEC_BIT_DELAY); // give ROM time to run through LE99C/LFF20/reach LE9DF

    // Step 2: assert CLK → LE9DF (waiting for PB2=1) exits → ROM enters LEA0B bit loop.
    iec_set_clk(drive, true);
    iec_run(drive, IEC_BIT_DELAY); // setup: ROM must reach LEA0B and be in CLK-high wait

    // Step 3: send 8 bits LSB-first
    // DATA HIGH (released, not asserted) = bit 1; DATA LOW (asserted) = bit 0.
    // LEA0B exits on CLK released (PB2=0 = host CLK only); LEA1A exits on CLK asserted.
    for bit_idx in 0..8u8 {
        let bit = (byte >> bit_idx) & 1 != 0;
        iec_set_data(drive, !bit);       // bit=1 → DATA released (not asserted)
        iec_set_clk(drive, false);       // CLK released = bit valid on wire
        iec_run(drive, IEC_BIT_DELAY);   // hold: ROM (LEA0B) must sample DATA
        iec_set_clk(drive, true);        // CLK asserted = bit done
        iec_run(drive, IEC_BIT_DELAY);   // hold: ROM (LEA1A) must register edge
    }

    // Step 4: release DATA; delay for ROM to process byte and re-enter LE9CD.
    // The 1541 does not assert DATA as an ACK between ATN-phase command bytes.
    iec_set_data(drive, false);
    iec_run(drive, IEC_BIT_DELAY);
    Ok(())
}

// ─── Byte receive (host is LISTENER, drive is TALKER) ─────────────────────────

/// Receive all bytes until the drive signals EOI (no more data).
///
/// The 1541-II ROM talker protocol (LE916/LE94B/LE95C) uses CLK=bit value and
/// DATA=strobe (released=valid, asserted=done via LFEFB).
///
/// Per-byte handshake:
///   1. Wait for bus DATA=0: drive cleared DATA_OUT (LE9B7), approaching LE941.
///      This naturally covers the initial LD0EB setup delay and, on a disk
///      channel, the motor spin-up (~500 000 cycles) and GCR decoding.
///   2. Probe: host asserts DATA → if drive is at LE941 it exits to LE94B,
///      calls LE9AE (DATA_OUT=1), then waits for PB0=0.  Host releases DATA.
///      If bus DATA is still 1 (drive holding it): genuine response, receive byte.
///      If bus DATA is 0 (drive at LEBE7 main loop, ignored probe): EOI, stop.
///   3. Receive 8 bits (LE95C): DATA released (LE9B7) = bit valid, CLK = bit;
///      DATA asserted + CLK (LFEFB) = bit done.
///   4. Byte ack: host asserts DATA briefly (LE987 polls PB0=1).
///      Outer loop restarts at step 1, waiting for LE9B7 of the next byte.
pub fn iec_receive_data(drive: &mut Emulator<HostBridge>) -> Result<Vec<u8>> {
    // Talker turnaround: host releases CLK and DATA.
    iec_set_clk(drive, false);
    iec_set_data(drive, false);

    let mut result = Vec::new();

    loop {
        // Step 1: wait for drive to clear DATA_OUT.
        // After LE8ED→LE9AE→LD0EB, the drive calls LE9B7 (DATA=0) then reaches
        // LE941.  On the first byte this covers initial setup; on subsequent bytes
        // it covers inter-byte re-initialisation (and disk I/O on file channels).
        eprintln!("[iec] waiting for DATA=0... bus.data={} ORB=${:02X} PC=${:04X}",
            iec_bus(drive).data() as u8, drive.cpu.bus.via1.orb, drive.cpu.pc);
        iec_wait(drive, |b| !b.data())?;
        eprintln!("[iec] DATA=0 seen: ORB=${:02X} PC=${:04X}", drive.cpu.bus.via1.orb, drive.cpu.pc);
        iec_run(drive, IEC_BIT_DELAY); // settling: drive completes LE937 → LE941

        // Step 2: probe.
        iec_set_data(drive, true);
        iec_run(drive, IEC_BIT_DELAY); // ≥1 LE941 polling iteration guaranteed
        iec_set_data(drive, false);
        iec_run(drive, 2);             // refresh bus_state; LE9B7 is ~30+ cycles away

        eprintln!("[iec] after probe release: bus.data={} ORB=${:02X} PC=${:04X}",
            iec_bus(drive).data() as u8, drive.cpu.bus.via1.orb, drive.cpu.pc);
        if !iec_bus(drive).data() {
            eprintln!("[iec] EOI detected (no probe response)");
            break; // EOI: drive at LEBE7, did not assert DATA_OUT
        }

        // Step 3: receive 8 bits LSB-first.
        // Drive is in LE94B / transitioning to LE95C (DATA_OUT=1, saw PB0=0).
        let mut byte_val: u8 = 0;
        for bit_idx in 0..8u8 {
            iec_wait(drive, |b| !b.data())?; // DATA released (LE9B7) = bit valid
            if iec_bus(drive).clk() { byte_val |= 1 << bit_idx; }
            iec_wait(drive, |b| b.data())?;  // DATA asserted (LFEFB) = bit done
        }
        eprintln!("[iec] received byte #{}: 0x{:02X}", result.len(), byte_val);
        result.push(byte_val);

        if byte_val == b'\r' {
            break; // end of status-channel message
        }

        // Step 4: byte ack — LE987 polls PB0=1, then advances to next byte.
        iec_set_data(drive, true);
        iec_run(drive, IEC_BIT_DELAY);   // LE987 sees PB0=1, exits → LD3AA
        iec_set_data(drive, false);
        eprintln!("[iec] after byte ack release: bus.data={} ORB=${:02X} PC=${:04X}",
            iec_bus(drive).data() as u8, drive.cpu.bus.via1.orb, drive.cpu.pc);
        // Outer loop restarts with iec_wait(!b.data()), waiting for LE9B7 of
        // the next byte (or detecting EOI if drive routes to LEBE7 instead).
    }

    Ok(result)
}

// ─── High-level IEC commands ─────────────────────────────────────────────────

/// Read the 1541 status channel (device 8, secondary address 15).
pub fn iec_read_status(drive: &mut Emulator<HostBridge>) -> Result<String> {
    iec_atn_start(drive)?;
    iec_send_byte(drive, 0x48)?; // TALK device 8
    iec_send_byte(drive, 0x6F)?; // DATA channel 15 (status)
    iec_atn_release(drive);

    let data = iec_receive_data(drive)?;

    iec_idle(drive);
    Ok(String::from_utf8_lossy(&data).trim_end_matches('\r').to_string())
}

/// Load a file by name from the 1541 using LOAD protocol.
/// Filename "$" retrieves the directory as a BASIC program.
pub fn iec_load_file(drive: &mut Emulator<HostBridge>, filename: &str) -> Result<Vec<u8>> {
    // 1. LISTEN + OPEN channel 0, then send filename
    iec_atn_start(drive)?;
    iec_send_byte(drive, 0x28)?; // LISTEN device 8
    iec_send_byte(drive, 0xF0)?; // OPEN secondary address 0
    iec_atn_release(drive);

    // Host sends filename bytes (host is still sender, drive is listener)
    for &b in filename.as_bytes() {
        iec_send_byte(drive, b)?;
    }

    // 2. UNLISTEN
    iec_atn_start(drive)?;
    iec_send_byte(drive, 0x3F)?; // UNLISTEN
    iec_atn_release(drive);

    // 3. TALK + DATA channel 0
    iec_atn_start(drive)?;
    iec_send_byte(drive, 0x48)?; // TALK device 8
    iec_send_byte(drive, 0x60)?; // DATA secondary address 0
    iec_atn_release(drive);

    let data = iec_receive_data(drive)?;

    iec_idle(drive);
    Ok(data)
}

// ─── Directory parser ─────────────────────────────────────────────────────────

/// Parse the raw BASIC-format directory listing returned by LOAD "$".
/// The data starts with a 2-byte load address, followed by BASIC line records.
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

        // Collect PETSCII bytes until null terminator
        let mut text = String::new();
        while pos < data.len() && data[pos] != 0x00 {
            let c = data[pos];
            let ascii: u8 = match c {
                0x20..=0x5F => c,
                0x60..=0x7E => c - 0x20,    // PETSCII shifted lowercase → uppercase
                0xC1..=0xDA => c - 0x80,    // PETSCII uppercase shifted
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
