//! Integration tests for the 1541 IEC serial bus emulation.
//!
//! These tests boot the real 1541-II ROM, then exercise the IEC protocol
//! against the emulated drive. Each test is progressive:
//!
//!   1. `test_drive_boots`       — ROM executes without panic for 2M cycles
//!   2. `test_atn_acknowledged`  — ATN assertion is acknowledged (DATA goes LOW)
//!   3. `test_status_response`   — Full TALK+channel-15 exchange returns DOS status
//!
//! All tests use the exact same setup as the debugger (HostBridge + Emulator).

use crossbeam_channel::unbounded;
use pico1541_debugger::{
    bridge::HostBridge,
    emulation::ROM_BYTES,
    iec::{iec_atn_start, iec_atn_release, iec_idle, iec_run, iec_read_status, IEC_TIMEOUT},
};
use pico1541_emulator::Emulator;

// ─── Test helpers ─────────────────────────────────────────────────────────────

fn make_drive() -> Emulator<HostBridge> {
    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);
    let (event_tx, _event_rx) = unbounded();
    let mut drive = Emulator::new(rom, HostBridge::new(event_tx));
    drive.reset();
    drive
}

/// Boot the drive by running cycles until the ROM is ready.
/// The 1541-II ROM takes roughly 500 000 cycles for self-test + LED flash.
/// We run 2 000 000 to be safe.
fn boot_drive(drive: &mut Emulator<HostBridge>) {
    iec_run(drive, 2_000_000);
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// Sanity check: the ROM executes 2M cycles without panicking.
/// If this fails something is fundamentally wrong with Emulator setup.
#[test]
fn test_drive_boots() {
    let mut drive = make_drive();
    boot_drive(&mut drive);
    // No assertion needed — reaching here means no panic/crash.
}

/// After ATN is asserted, the drive must pull DATA LOW within IEC_TIMEOUT cycles.
/// This is the first thing the 1541 ROM does when it detects ATN via PB4 / CA1.
/// If this times out, the VIA1 bit-mapping or CA1 edge detection is wrong.
#[test]
fn test_atn_acknowledged() {
    let mut drive = make_drive();
    boot_drive(&mut drive);

    iec_idle(&mut drive);
    iec_atn_start(&mut drive).expect("Drive did not acknowledge ATN (DATA never went LOW)");
    iec_atn_release(&mut drive);
    iec_idle(&mut drive);
}

/// Full IEC TALK exchange on channel 15 (status channel).
/// After boot the 1541 reports: "73,CBM DOS V2.6 1541,00,00"
/// (decimal 73 = power-on / DOS reset message).
#[test]
fn test_status_response() {
    let mut drive = make_drive();
    boot_drive(&mut drive);

    let status = iec_read_status(&mut drive)
        .expect("IEC status request timed out");

    println!("Drive status: {:?}", status);

    assert!(!status.is_empty(), "Status string was empty");

    // The first field is always the error number.
    // On power-on/reset it is "73"; after a successful op it would be "00".
    let error_num: u8 = status
        .split(',')
        .next()
        .and_then(|s| s.trim().parse().ok())
        .expect("Status did not start with a numeric error code");

    assert!(
        error_num == 73 || error_num == 0,
        "Unexpected error number {} in status: {:?}", error_num, status
    );
}

/// Verify IEC_TIMEOUT is exported so callers can tune wait loops.
#[test]
fn test_iec_timeout_constant_exported() {
    assert!(IEC_TIMEOUT >= 1_000_000, "IEC_TIMEOUT should be at least 1 M cycles");
}
