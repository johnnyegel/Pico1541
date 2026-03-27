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
//! IEC bus convention (compact IecBus format):
//!   bit 0 = DATA  (1=HIGH/released, 0=LOW/asserted)
//!   bit 1 = CLK   (1=HIGH/released, 0=LOW/asserted)
//!   bit 2 = ATN   (1=HIGH/released, 0=LOW/asserted)
//!
//! VIA1 convention (active-HIGH inputs, inverted from bus):
//!   PB7 = DATA_IN  (1 when DATA is LOW)
//!   PB6 = CLK_IN   (1 when CLK is LOW)
//!   PB3 = ATN_IN   (1 when ATN is LOW)
//!   PB0 = DATA_OUT drive output (1 = transistor pulls DATA LOW)
//!   PB2 = CLK_OUT  drive output (1 = transistor pulls CLK LOW)

use anyhow::{anyhow, Result};
use pico1541_emulator::{interface::IecBus, Emulator};

use crate::bridge::HostBridge;

/// Cycles per half-bit phase. The 1541 ROM polling loop is ~10–15 cycles/iter;
/// 200 cycles gives it ~15+ iterations to sample each signal edge.
const IEC_BIT_DELAY: u32 = 200;

/// Settling/turnaround delay used after ATN changes and talker handshakes.
const IEC_SETUP_DELAY: u32 = 600;

/// Cycles CLK must stay HIGH from the drive before we treat it as EOI.
/// 400 cycles ≈ 400 µs at 1 MHz, safely above the 200 µs spec threshold.
const IEC_EOI_THRESHOLD: u32 = 400;

/// Maximum cycles to wait for any bus event before declaring a timeout.
const IEC_TIMEOUT: u32 = 2_000_000; // 2 seconds at 1 MHz

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

pub fn iec_set_data(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_data(high);
}
pub fn iec_set_clk(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_clk(high);
}
pub fn iec_set_atn(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_atn(high);
}

/// Run the emulator until `pred(bus_state)` returns true, or IEC_TIMEOUT cycles.
pub fn iec_wait(drive: &mut Emulator<HostBridge>, pred: impl Fn(IecBus) -> bool) -> Result<()> {
    for _ in 0..IEC_TIMEOUT {
        iec_tick(drive);
        if pred(iec_bus(drive)) { return Ok(()); }
    }
    Err(anyhow!("IEC timeout"))
}

// ─── Handshake ────────────────────────────────────────────────────────────────

/// Assert ATN and wait for the drive to acknowledge by pulling DATA LOW.
///
/// The 1541 ROM responds two ways:
///   • CA1 interrupt: fires on ATN's *falling edge* (HIGH→LOW transition).
///     `via1.last_ca1` must be `true` before we pull ATN LOW, otherwise the
///     edge detector sees false→false and no IRQ fires.  We guarantee this by
///     running a few cycles with ATN=HIGH first.
///   • PB4 polling: the ROM's main loop also reads ATN_IN (PB4) directly.
///     This is the fallback if the interrupt was already missed.
pub fn iec_atn_start(drive: &mut Emulator<HostBridge>) -> Result<()> {
    // Release ATN and run a few cycles so CA1 sees HIGH → last_ca1 = true
    iec_set_atn(drive, true);
    iec_set_clk(drive, false); // host holds CLK throughout
    iec_run(drive, 20);        // just enough to register the HIGH on CA1

    // Now assert ATN — this creates a clean HIGH→LOW falling edge on CA1
    iec_set_atn(drive, false);
    iec_run(drive, IEC_SETUP_DELAY);
    iec_wait(drive, |b| !b.data())
}

/// Release ATN and run a settling delay.
pub fn iec_atn_release(drive: &mut Emulator<HostBridge>) {
    iec_set_atn(drive, true);
    iec_run(drive, IEC_SETUP_DELAY);
}

/// Release the bus to idle (all lines HIGH/released).
pub fn iec_idle(drive: &mut Emulator<HostBridge>) {
    iec_set_data(drive, true);
    iec_set_clk(drive, true);
    iec_set_atn(drive, true);
}

// ─── Byte send (host is SENDER) ───────────────────────────────────────────────

/// Send one byte.  ATN may be asserted or released; the protocol is the same.
/// The 1541 ROM byte-receive routine (GBIT) expects:
///   1. CLK HIGH  → ROM releases DATA ("ready to receive")
///   2. CLK LOW   → ROM enters bit-read loop ("sender about to send")
///   3. CLK HIGH  → bit N valid on DATA; CLK LOW → bit done  (×8)
///   4. After bit 7: ROM asserts DATA (byte acknowledge)
pub fn iec_send_byte(drive: &mut Emulator<HostBridge>, byte: u8) -> Result<()> {
    // Step 1: release CLK → ROM releases DATA
    iec_set_clk(drive, true);
    iec_wait(drive, |b| b.data())?; // wait for drive DATA HIGH (ready)

    // Step 2: assert CLK → ROM transitions to bit-read loop
    iec_set_clk(drive, false);
    iec_run(drive, IEC_BIT_DELAY * 2); // generous setup: ROM must reach "wait for CLK HIGH"

    // Step 3: send 8 bits LSB-first
    for bit_idx in 0..8u8 {
        let bit = (byte >> bit_idx) & 1 != 0;
        iec_set_data(drive, bit);        // put bit on DATA line
        iec_set_clk(drive, true);        // CLK HIGH = bit valid
        iec_run(drive, IEC_BIT_DELAY);   // hold: ROM must sample DATA
        iec_set_clk(drive, false);       // CLK LOW = bit done
        iec_run(drive, IEC_BIT_DELAY);   // hold: ROM must register edge
    }

    // Step 4: release DATA; wait for drive to pull DATA LOW (byte acknowledge)
    iec_set_data(drive, true);
    iec_wait(drive, |b| !b.data())?;

    // Reassert CLK for next byte (host still controls it)
    iec_set_clk(drive, false);
    iec_run(drive, IEC_BIT_DELAY);
    Ok(())
}

// ─── Byte receive (host is LISTENER, drive is TALKER) ─────────────────────────

/// Receive all bytes until the drive signals EOI (last byte).
///
/// After ATN is released with a TALK command the drive performs the
/// talker-turnaround: it asserts CLK (holding) then releases CLK per byte.
/// The host signals "ready" by releasing DATA, then reads bits on each
/// CLK HIGH phase.
pub fn iec_receive_data(drive: &mut Emulator<HostBridge>) -> Result<Vec<u8>> {
    // Talker turnaround: host releases CLK so drive can take it
    iec_set_clk(drive, true);
    iec_set_data(drive, true);
    iec_run(drive, IEC_SETUP_DELAY); // give drive time to assert CLK and set up

    let mut result = Vec::new();

    loop {
        // --- per-byte receive ---

        // Brief DATA=LOW ("not ready yet"), then release to signal "ready"
        iec_set_data(drive, false);
        iec_run(drive, IEC_BIT_DELAY);
        iec_set_data(drive, true); // ready to receive

        // Wait for drive to release CLK (HIGH) = "I have a byte for you"
        // The drive holds CLK LOW when idle / not ready to send.
        iec_wait(drive, |b| b.clk())?;

        // EOI detection: count cycles CLK stays HIGH before first bit.
        // If drive keeps CLK HIGH past IEC_EOI_THRESHOLD, this is the last byte.
        let mut clk_high_cycles = 0u32;
        let mut eoi = false;

        loop {
            iec_tick(drive);
            if !iec_bus(drive).clk() {
                // CLK went LOW: drive starting bit transmission (no EOI yet)
                break;
            }
            clk_high_cycles += 1;
            if clk_high_cycles > IEC_EOI_THRESHOLD {
                // EOI: acknowledge by pulsing DATA LOW ~60 µs, then release
                iec_set_data(drive, false);
                iec_run(drive, 60);
                iec_set_data(drive, true);
                eoi = true;
                // Wait for drive to assert CLK (start sending after EOI ack)
                iec_wait(drive, |b| !b.clk())?;
                break;
            }
        }

        // Read 8 bits LSB-first.
        // Drive: CLK HIGH = bit valid on DATA; CLK LOW = bit done.
        let mut byte_val: u8 = 0;
        for bit_idx in 0..8u8 {
            iec_wait(drive, |b| b.clk())?;  // CLK HIGH: bit is valid
            if iec_bus(drive).data() { byte_val |= 1 << bit_idx; }
            iec_wait(drive, |b| !b.clk())?; // CLK LOW: bit done
        }
        result.push(byte_val);

        // Acknowledge: assert DATA briefly, release
        iec_set_data(drive, false);
        iec_run(drive, 60);
        iec_set_data(drive, true);

        if eoi { break; }
    }

    Ok(result)
}

// ─── High-level IEC commands ─────────────────────────────────────────────────

/// Read the 1541 status channel (device 8, secondary address 15).
pub async fn iec_read_status(drive: &mut Emulator<HostBridge>) -> Result<String> {
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
pub async fn iec_load_file(drive: &mut Emulator<HostBridge>, filename: &str) -> Result<Vec<u8>> {
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
