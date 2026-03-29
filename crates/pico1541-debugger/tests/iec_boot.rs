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
use pico1541_common::{D64Image, DiskImage, MemoryStorage};
use pico1541_debugger::{
    bridge::HostBridge,
    emulation::ROM_BYTES,
    iec::{iec_atn_start, iec_atn_release, iec_idle, iec_load_file, iec_run, iec_read_status,
          parse_directory, IEC_TIMEOUT},
};
use pico1541_emulator::{interface::{HostInterface, MAX_TRACK_BYTES}, Emulator};

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

/// Build a minimal 174 848-byte D64 image in memory.
///
/// Track 18 sector 0 = BAM:  disk name "TEST DISK", disk ID "T1".
/// Track 18 sector 1 = directory:  one file "HELLO" (PRG, 1 block).
fn make_minimal_d64() -> Vec<u8> {
    const D64_SIZE: usize = 174_848;
    // Track 18 starts after tracks 1-17 (17 tracks × 21 sectors × 256 bytes).
    const T18_OFFSET: usize = 17 * 21 * 256; // = 91 392

    let mut d64 = vec![0u8; D64_SIZE];

    // ── BAM (sector 0) ──────────────────────────────────────────────────────
    let bam = &mut d64[T18_OFFSET..T18_OFFSET + 256];
    bam[0] = 18;   // track of first directory sector
    bam[1] = 1;    // sector of first directory sector
    bam[2] = 0x41; // DOS format version 'A'
    // Disk name at $90 (16 PETSCII bytes, padded with $A0)
    let name = b"TEST DISK";
    bam[0x90..0x90 + name.len()].copy_from_slice(name);
    for b in &mut bam[0x90 + name.len()..0xA0] { *b = 0xA0; }
    bam[0xA0] = 0xA0;
    bam[0xA1] = 0xA0;
    bam[0xA2] = b'T'; // disk ID byte 0
    bam[0xA3] = b'1'; // disk ID byte 1
    bam[0xA4] = 0xA0;
    bam[0xA5] = b'2';
    bam[0xA6] = b'A';

    // ── Directory sector 1 ──────────────────────────────────────────────────
    let dir = &mut d64[T18_OFFSET + 256..T18_OFFSET + 512];
    dir[0] = 0;    // no next directory sector (last sector)
    dir[1] = 0xFF;
    // Entry 0 (32-byte entry starting at byte 2 within the sector):
    dir[2] = 0x82; // PRG, closed
    dir[3] = 17;   // track of first data block (any valid track)
    dir[4] = 0;    // sector of first data block
    // Filename: "HELLO" + $A0 padding (16 bytes starting at offset 5)
    let fname = b"HELLO";
    for b in &mut dir[5..21] { *b = 0xA0; }
    dir[5..5 + fname.len()].copy_from_slice(fname);
    // File size in 256-byte blocks (little-endian at entry offset 28 = sector offset 30)
    dir[30] = 1;
    dir[31] = 0;

    d64
}

/// Full IEC LOAD "$" exchange after mounting a disk with a known directory.
///
/// Validates that:
///   1. The motor-spin-up wait in `iec_receive_data` correctly handles the
///      ~500 000-cycle delay before the drive can deliver GCR data.
///   2. The directory listing contains the expected disk name and file name.
#[test]
fn test_dir_after_mount() {
    let d64_data = make_minimal_d64();
    let disk_id = [b'T', b'1'];

    // Build GCR for track 18 from the in-memory D64 image.
    let storage = MemoryStorage::new(&d64_data);
    let mut img = D64Image::new(storage);
    img.set_id(disk_id);

    let mut gcr_buf = vec![0u8; MAX_TRACK_BYTES];
    let bit_len = futures::executor::block_on(
        DiskImage::read_track(&mut img, 36, &mut gcr_buf)
    ).expect("read_track failed");

    // Boot the drive without a disk inserted.
    let mut drive = make_drive();
    boot_drive(&mut drive);

    // Mount: load track 18 GCR into the active track buffer and mark disk present.
    // Also align the emulated head position so the ROM finds valid track-18 sector headers.
    {
        let track = drive.cpu.bus.hardware.get_active_track();
        track.gcr_data.copy_from_slice(&gcr_buf);
        track.bit_length = bit_len;
        track.disk_inserted = true;
    }
    drive.cpu.bus.mechanics.current_half_track = 36; // head at track 18

    // Request the directory (LOAD "$").
    let dir_data = iec_load_file(&mut drive, "$").expect("iec_load_file timed out");
    let dir_text = parse_directory(&dir_data);

    println!("Directory listing:\n{}", dir_text);

    assert!(
        dir_text.contains("TEST DISK"),
        "Expected disk name 'TEST DISK' in directory output, got:\n{}",
        dir_text
    );
    assert!(
        dir_text.contains("HELLO"),
        "Expected file 'HELLO' in directory output, got:\n{}",
        dir_text
    );
}
