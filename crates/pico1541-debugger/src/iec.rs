//! Commodore IEC serial bus protocol implementation.
//!
//! The host (debugger) emulates the C64 side of the IEC bus.
//! All IEC helpers run the emulator themselves cycle-by-cycle, so they must be
//! called with the emulator paused (the regular run loop must not be ticking
//! concurrently).
//!
//! ## Synchronization strategy
//!
//! Synchronization is **state-based** rather than cycle-count-based wherever
//! possible.  `iec_wait` polls bus lines (open-drain conditions observable on
//! the wire).  `iec_wait_state` polls the drive's CPU program counter, letting
//! the host wait for the ROM to reach a specific protocol state before
//! asserting or releasing a line.
//!
//! The 1541-II ROM talker state machine has two paths after the inter-byte
//! handshake, depending on `$F2[ch]` bit 3:
//!
//!   * **Normal path** (`$F2 bit 3 = 0`): drive reaches `LE941` and spins
//!     waiting for the host to assert DATA (the "probe").  If no probe arrives,
//!     that signals EOI.
//!
//!   * **Fast path** (`$F2 bit 3 = 1`, used by status channel 15): drive jumps
//!     directly to `LE94B`, asserts DATA_OUT=1, and waits for DATA_IN=0 without
//!     ever visiting `LE941`.  A probe here would be interpreted as a DATA
//!     abort by `LE95C`.
//!
//! The state-based helpers detect which path the drive took and respond
//! correctly for both cases.
//!
//! ## ROM addresses (1541-II 251968-03)
//!
//! These constants are specific to this ROM version.  If support for other
//! ROM versions is added, they should be parameterised.
//!
//! IEC bus convention (open-drain, asserted = LOW):
//!   DATA asserted  = true  (LOW on bus)
//!   DATA released  = false (HIGH / floating)

use anyhow::{anyhow, Result};
use pico1541_emulator::interface::IecBus;
use pico1541_emulator::Emulator;

use crate::bridge::{BusSnapshot, HostBridge};

/// Settling/turnaround delay used after ATN changes and initial talker setup.
const IEC_SETUP_DELAY: u32 = 2000;

/// Cycles per half-bit phase when the host is sending bytes.
/// The 1541 ROM listener polling loop is ~10–15 cycles/iter;
/// 1000 cycles gives it plenty of iterations to sample each signal edge.
const IEC_BIT_DELAY: u32 = 1000;

/// Maximum cycles to wait for any bus event before declaring a timeout.
pub const IEC_TIMEOUT: u32 = 10_000_000; // 10 seconds at 1 MHz

// ─── 1541-II ROM talker state addresses ──────────────────────────────────────
//
// These mark key points in the 1541-II talker send loop (LE916–LE987).

/// Drive is in the probe-wait loop: spins while DATA_IN=0, exits on probe.
const ROM_TALKER_PROBE_WAIT: core::ops::RangeInclusive<u16> = 0xE941..=0xE94A;

/// Drive responded to probe (or fast-pathed via $F2 bit 3): DATA_OUT=1,
/// waiting for DATA_IN=0 (host to release its probe).
const ROM_TALKER_RESPONDED: core::ops::RangeInclusive<u16> = 0xE94B..=0xE95B;

/// Drive is sending bit data (LE95C loop).
#[allow(dead_code)]
const ROM_TALKER_SENDING: core::ops::RangeInclusive<u16> = 0xE95C..=0xE986;

/// Drive is in the byte-ack wait loop: spins while DATA_IN=0, exits on ack.
const ROM_TALKER_ACK_WAIT: core::ops::RangeInclusive<u16> = 0xE987..=0xE98E;

/// Drive is in the post-ack fast-path release loop (`LE925`): arrived here from
/// `LE916` because DATA_IN=1 (ack line still held).  Spins while DATA_IN=1, then
/// checks `$F2[ch]` bit 3 → jumps to `LE94B` (fast) or falls through to `LE937`
/// (normal).  Releasing DATA while the drive is here lets it exit cleanly.
///
/// Range: $E925 (jsr LEA59) … $E936 (second byte of `bne LE94B`).
const ROM_TALKER_FAST_LOOP: core::ops::RangeInclusive<u16> = 0xE925..=0xE936;

// ─── 1541-II ROM listener state addresses ────────────────────────────────────
//
// Key addresses in the 1541-II listener receive loop (LE9C9/LE9DF/LEA0B).

/// Drive reached ready state: waiting for CLK assert (host is about to send).
#[allow(dead_code)]
const ROM_LISTENER_READY: core::ops::RangeInclusive<u16> = 0xE9DF..=0xE9E5;

/// Drive is in the bit-receive loop: exits on CLK released (bit valid).
#[allow(dead_code)]
const ROM_LISTENER_BIT_VALID: core::ops::RangeInclusive<u16> = 0xEA0B..=0xEA19;

/// Drive is waiting for CLK asserted (bit done strobe).
#[allow(dead_code)]
const ROM_LISTENER_BIT_DONE: core::ops::RangeInclusive<u16> = 0xEA1A..=0xEA2A;

// ─── Low-level helpers ────────────────────────────────────────────────────────

pub fn iec_tick(drive: &mut Emulator<HostBridge>) {
    drive.cpu.tick();
}

pub fn iec_run(drive: &mut Emulator<HostBridge>, n: u32) {
    for _ in 0..n { drive.cpu.tick(); }
}

/// Combined bus state as last written by the emulator's open-drain model.
#[inline]
pub fn iec_bus(drive: &mut Emulator<HostBridge>) -> BusSnapshot {
    let bus = drive.cpu.bus.hardware.c64_bus();
    BusSnapshot { data: bus.read_data(), clk: bus.read_clk(), atn: bus.read_atn() }
}

pub fn iec_set_data(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.c64_bus().drive_data(asserted);
}
pub fn iec_set_clk(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.c64_bus().drive_clk(asserted);
}
pub fn iec_set_atn(drive: &mut Emulator<HostBridge>, asserted: bool) {
    drive.cpu.bus.hardware.c64_bus().set_atn(asserted);
}

/// Run the emulator until `pred(bus_state)` returns true, or `timeout` cycles.
pub fn iec_wait_timeout(drive: &mut Emulator<HostBridge>, timeout: u32, pred: impl Fn(BusSnapshot) -> bool) -> Result<()> {
    for _ in 0..timeout {
        iec_tick(drive);
        if pred(iec_bus(drive)) { return Ok(()); }
    }
    let pc = drive.cpu.pc;
    Err(anyhow!("IEC timeout (CPU PC = ${:04X})", pc))
}

pub fn iec_wait(drive: &mut Emulator<HostBridge>, pred: impl Fn(BusSnapshot) -> bool) -> Result<()> {
    iec_wait_timeout(drive, IEC_TIMEOUT, pred)
}

/// Run the emulator until `pred(&drive)` returns true, or `timeout` cycles.
///
/// Unlike `iec_wait`, which observes bus lines, `iec_wait_state` can inspect
/// any part of the emulator — most usefully `drive.cpu.pc` — to synchronize
/// on a specific ROM protocol state.
pub fn iec_wait_state(
    drive: &mut Emulator<HostBridge>,
    timeout: u32,
    pred: impl Fn(&Emulator<HostBridge>) -> bool,
) -> Result<()> {
    for _ in 0..timeout {
        iec_tick(drive);
        if pred(drive) { return Ok(()); }
    }
    let pc = drive.cpu.pc;
    Err(anyhow!("IEC state timeout (CPU PC = ${:04X})", pc))
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
    iec_wait(drive, |b| b.data)
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
/// Uses cycle-based timing (`IEC_BIT_DELAY`) for the send path.  The 1541
/// listener ROM samples bus lines in a tight polling loop; giving it
/// `IEC_BIT_DELAY` cycles per phase is sufficient for reliable reception.
///
/// Protocol (1541-II listener, LE9C9/LE9DF/LEA0B):
///   1. CLK released (IEC_BIT_DELAY cycles) → drive detects CLK HIGH.
///   2. CLK asserted (IEC_BIT_DELAY cycles) → drive is ready for first bit.
///   3. For each bit (LSB first):
///      - Set DATA to bit value (DATA released = 1, asserted = 0).
///      - Release CLK (IEC_BIT_DELAY) → drive samples DATA.
///      - Assert CLK (IEC_BIT_DELAY) → drive advances to next bit.
///   4. Release DATA; run a settling delay.
pub fn iec_send_byte(drive: &mut Emulator<HostBridge>, byte: u8) -> Result<()> {
    iec_set_clk(drive, false);
    iec_run(drive, IEC_BIT_DELAY);
    iec_set_clk(drive, true);
    iec_run(drive, IEC_BIT_DELAY);
    for bit_idx in 0..8u8 {
        let bit = (byte >> bit_idx) & 1 != 0;
        iec_set_data(drive, !bit); // bit=1 → DATA released; bit=0 → DATA asserted
        iec_set_clk(drive, false);
        iec_run(drive, IEC_BIT_DELAY);
        iec_set_clk(drive, true);
        iec_run(drive, IEC_BIT_DELAY);
    }
    iec_set_data(drive, false);
    iec_run(drive, IEC_BIT_DELAY);
    Ok(())
}

// ─── Byte receive (host is LISTENER, drive is TALKER) ─────────────────────────

/// Receive all bytes until the drive signals EOI (no more data).
///
/// Synchronizes on the drive's CPU state (ROM address) to handle both talker
/// paths cleanly:
///
///   **Normal path** (`$F2 bit 3 = 0`): drive reaches `LE941` and waits for
///   the host to assert DATA (probe).  Probe causes drive to enter `LE94B`
///   (DATA_OUT=1), then `LE95C` (bit send).
///
///   **Fast path** (`$F2 bit 3 = 1`, e.g. status channel 15): drive reaches
///   `LE94B` directly via the inter-byte ack path (DATA_IN=1 at `LE916`
///   → `LE925` → `LE94B`) without visiting `LE941`.  Do NOT send a probe
///   here — the abort check at the top of `LE95C` would see DATA_IN=1 and
///   abort the transfer.
///
/// ## Per-byte handshake
///
///   1. Wait for `LE941` (normal) or `LE94B` (fast).  EOI: drive returns to
///      its main loop and these addresses are never reached → timeout → break.
///   2. If `LE941`: assert DATA (probe), wait for `LE94B`, release DATA.
///      `LE94B` then exits to `LE95C` (DATA_IN=0 after release).
///      If `LE94B`: DATA_IN=0 → `LE94B` exits immediately; no probe needed.
///   3. Receive 8 bits LSB-first.  `LE9B7` releases DATA_OUT (bit valid,
///      CLK encodes bit value); `LFEFB` asserts DATA_OUT+CLK_OUT (bit done).
///   4. Byte ack: wait for `LE987` (drive looping on DATA_IN=0), assert DATA
///      → drive exits `LE987` and traverses `LE916` → `LE925`.  `LE925`
///      spins while DATA_IN=1; detect it and release DATA.  Drive exits
///      `LE925` and heads to `LE94B` (fast) or `LE941` (normal) — caught by
///      step 1 on the next iteration.
pub fn iec_receive_data(drive: &mut Emulator<HostBridge>) -> Result<Vec<u8>> {
    // Talker turnaround: host releases CLK and DATA.
    iec_set_clk(drive, false);
    iec_set_data(drive, false);

    let mut result = Vec::new();

    loop {
        // Step 1: wait for a known talker-ready state.
        //
        // First byte: drive reaches LE941 (probe-wait) because DATA_IN=0 when
        //   LE916 first runs (host had DATA released after the ATN phase).
        // Subsequent bytes (fast path): drive reaches LE94B after the ack
        //   (LE916 sees DATA_IN=1 → LE925 → LE94B).
        // Subsequent bytes (normal path): drive reaches LE941 after LE925 exit
        //   with $F2 bit 3 = 0 (falls through LE937 → LE941).
        //
        // EOI: drive returns to main loop; neither address is reached →
        //   timeout → treat as end-of-stream.
        let reached = iec_wait_state(drive, IEC_TIMEOUT, |d| {
            let pc = d.cpu.pc;
            ROM_TALKER_PROBE_WAIT.contains(&pc) || ROM_TALKER_RESPONDED.contains(&pc)
        });
        if reached.is_err() {
            break; // EOI or no more data
        }

        // Step 2: probe (normal) or skip (fast path).
        if ROM_TALKER_PROBE_WAIT.contains(&drive.cpu.pc) {
            // LE941: assert DATA (probe) → drive exits LE941 → LE94B.
            // Wait for LE94B using a positive match only — using
            // "!ROM_TALKER_PROBE_WAIT" as the exit condition would fire on the
            // subroutine calls (jsr LEA59, jsr LE9C0) made from within the
            // LE941 loop, giving a false-positive before the loop actually exits.
            iec_set_data(drive, true);
            let responded = iec_wait_state(drive, IEC_TIMEOUT, |d| {
                ROM_TALKER_RESPONDED.contains(&d.cpu.pc)
            });
            iec_set_data(drive, false); // release probe → LE94B exits when DATA_IN=0
            if responded.is_err() {
                break; // no probe response → EOI
            }
        }
        // Both probe path (LE941→LE94B) and fast path (LE925→LE94B) arrive here
        // with PC somewhere in the LE94B block (E94B..E95B).  The first instruction
        // of LE94B is `jsr LE9AE` which asserts DATA_OUT.  `iec_wait_state` may
        // fire before that jsr executes, leaving bus_state.data still false.
        // Wait for DATA_OUT to be asserted before entering the bit loop; without
        // this, the first `!b.data` check fires immediately on a stale bus,
        // sampling CLK before the drive has placed any bit value on it.
        iec_wait(drive, |b| b.data)?;
        // Drive has now run LE9AE (DATA_OUT=1) and will exit the LE94B loop
        // (DATA_IN=0) and enter LE95C to begin bit transmission.

        // Step 3: receive 8 bits LSB-first.
        // Bit valid: drive releases DATA_OUT at LE9B7 (CLK already set to bit value).
        // Bit done:  drive asserts DATA_OUT + CLK at LFEFB.
        let mut byte_val: u8 = 0;
        for bit_idx in 0..8u8 {
            iec_wait(drive, |b| !b.data)?; // bit valid: DATA released
            if iec_bus(drive).clk { byte_val |= 1 << bit_idx; }
            iec_wait(drive, |b| b.data)?;  // bit done: DATA asserted
        }
        result.push(byte_val);

        if byte_val == b'\r' {
            // Ack the final byte so the drive cleanly exits LE987 back to its
            // main loop.  Without this the drive stays stuck in the LE987 spin
            // loop; on the next iec_atn_start the ATNA gate gives a false
            // DATA-ack, and the first 0-bit in the following TALK command
            // accidentally exits LE987, causing the drive to emit a spurious
            // byte at the start of the next status read.
            if let Ok(()) = iec_wait_state(drive, IEC_TIMEOUT, |d| ROM_TALKER_ACK_WAIT.contains(&d.cpu.pc)) {
                iec_set_data(drive, true);
                let _ = iec_wait_state(drive, IEC_TIMEOUT, |d| ROM_TALKER_FAST_LOOP.contains(&d.cpu.pc));
                iec_set_data(drive, false);
            }
            break;
        }

        // Step 4: byte ack.
        //
        // Drive is at LE987 looping while DATA_IN=0 (waiting for host ack).
        // Assert DATA → drive exits LE987 → LE916 → LE925.
        //
        // LE925 loops while DATA_IN=1 (the ack line we are holding), then checks
        // $F2[ch] bit 3:
        //   bit 3 = 1 (fast path): jumps to LE94B → LE95C (next byte immediately).
        //   bit 3 = 0 (normal):    falls through LE937 → LE941 (probe needed).
        //
        // We detect LE925 and release DATA there.  LE925 sees DATA_IN=0, exits,
        // and the drive heads toward LE94B or LE941 — caught by step 1 next iter.
        //
        // Do NOT use "!ROM_TALKER_ACK_WAIT" as the release trigger: subroutine
        // calls from within the LE987 loop (jsr LEA59, jsr LE9C0) take PC
        // outside that range before the loop's DATA_IN check runs, causing the
        // same false-positive pattern as the probe wait.
        iec_wait_state(drive, IEC_TIMEOUT, |d| ROM_TALKER_ACK_WAIT.contains(&d.cpu.pc))?;
        iec_set_data(drive, true);
        iec_wait_state(drive, IEC_TIMEOUT, |d| ROM_TALKER_FAST_LOOP.contains(&d.cpu.pc))?;
        iec_set_data(drive, false);
        // Drive will exit LE925 → LE94B or LE941; step 1 handles both.
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
