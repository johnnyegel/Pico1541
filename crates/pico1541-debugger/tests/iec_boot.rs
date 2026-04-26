//! Integration tests for the 1541-II DOS boot sequence and IEC protocol.
//!
//! These tests verify that the emulated 6502, VIAs, and disk mechanics are
//! sufficiently accurate to satisfy the original CBM 1541-II ROM (251968-03).
//!
//! Key areas verified:
//!   1. `test_drive_boots`       — Power-on self-test (POST) and initial head BUMP
//!   2. `test_dir_after_mount`   — LOAD "$" protocol + GCR head alignment + BAM parsing
//!   3. `test_status_response`   — Full TALK+channel-15 exchange returns DOS status
//!
//! All tests use the exact same setup as the debugger (HostBridge + Emulator).

use crossbeam_channel::unbounded;
use pico1541_common::{D64Image, DiskImage, MemoryStorage};
use pico1541_debugger::{
    bridge::HostBridge,
    emulation::ROM_BYTES,
    iec::{iec_idle, iec_load_file, iec_run, iec_read_status, parse_directory},
};
use pico1541_emulator::{interface::MAX_TRACK_BYTES, Emulator};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_drive() -> Emulator<HostBridge> {
    let (tx, _rx) = unbounded();
    let bridge = HostBridge::new(tx);
    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);
    Emulator::new(rom, bridge)
}

fn boot_drive(drive: &mut Emulator<HostBridge>) {
    // Ensure host lines are released before reset
    iec_idle(drive);
    drive.reset();
    // Run for 2 seconds of emulated time to allow POST and BUMP to complete.
    iec_run(drive, 2_000_000);
}

/// Create a minimal valid D64 image containing one file "HELLO".
fn make_minimal_d64() -> Vec<u8> {
    let mut data = vec![0u8; 174848]; // standard 35-track D64 size

    // Track 18, Sector 0: BAM (Block Availability Map)
    // T18S0 is at byte offset 91392 ($16500)
    let bam_off = 91392;
    data[bam_off + 0] = 18;  // pointer to first directory block (T18 S1)
    data[bam_off + 1] = 1;
    data[bam_off + 2] = b'A'; // DOS version 'A'
    data[bam_off + 160] = b'T'; // Disk Name "TEST DISK"
    data[bam_off + 161] = b'E';
    data[bam_off + 162] = b'S';
    data[bam_off + 163] = b'T';
    data[bam_off + 164] = b' ';
    data[bam_off + 165] = b'D';
    data[bam_off + 166] = b'I';
    data[bam_off + 167] = b'S';
    data[bam_off + 168] = b'K';
    for i in 169..186 { data[bam_off + i] = 0xA0; } // padded with shifted-space
    data[bam_off + 165 + 16] = b'1'; // ID "11"
    data[bam_off + 166 + 16] = b'1';

    // Track 18, Sector 1: Directory Block
    let dir_off = 91392 + 256;
    data[dir_off + 0] = 0;   // no further directory blocks
    data[dir_off + 1] = 0xFF;
    
    // First entry
    data[dir_off + 2] = 0x82; // PRG, closed
    data[dir_off + 3] = 17;   // T17 S0 (dummy file location)
    data[dir_off + 4] = 0;
    data[dir_off + 5] = b'H'; // "HELLO"
    data[dir_off + 6] = b'E';
    data[dir_off + 7] = b'L';
    data[dir_off + 8] = b'L';
    data[dir_off + 9] = b'O';
    for i in 10..21 { data[dir_off + i] = 0xA0; }
    
    data
}

// ─── Tests ───────────────────────────────────────────────────────────────────

/// Verifies the basic power-on boot sequence and BUMP works without crashing.
/// If this fails something is fundamentally wrong with Emulator setup.
#[test]
fn test_drive_boots() {
    let mut drive = make_drive();
    boot_drive(&mut drive);
    // No assertion needed — reaching here means no panic/crash.
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use pico1541_debugger::bridge::{BusSnapshot, LockStepCommand, LockStepResponse};
use pico1541_debugger::iec::{LockStepBus, IecDriver};

/// Verifies status channel 15 response using the LockStep mechanism.
#[test]
fn test_status_response() {
    let (tx, _rx) = unbounded();
    let mut bridge = HostBridge::new(tx);
    
    let (ls_cmd_tx, ls_cmd_rx) = unbounded();
    let (ls_resp_tx, ls_resp_rx) = unbounded();
    bridge.set_lockstep(ls_cmd_rx, ls_resp_tx);

    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);
    
    // Background emulation thread handling LockStep
    thread::spawn(move || {
        let mut drive = Emulator::new(rom, bridge);
        drive.reset();
        
        while running_clone.load(Ordering::SeqCst) {
            let ls_rx = drive.bus().hardware.lockstep_rx.clone();
            if let Some(rx) = ls_rx {
                if let Ok(cmd) = rx.try_recv() {
                    match cmd {
                        LockStepCommand::Step { host_drives, cycles } => {
                            drive.bus_mut().hardware.host_drives = host_drives;
                            for _ in 0..cycles { drive.cpu.tick(); }
                            let _ = drive.bus().hardware.lockstep_tx.as_ref().unwrap()
                                .send(LockStepResponse::Done(drive.bus().hardware.bus_state, drive.pc()));
                        }
                        LockStepCommand::Wait { host_drives, target, mask, timeout_cycles } => {
                            drive.bus_mut().hardware.host_drives = host_drives;
                            let mut timed_out = true;
                            for _ in 0..timeout_cycles {
                                drive.cpu.tick();
                                let bus = drive.bus().hardware.bus_state;
                                if (bus.data == target.data || !mask.data) &&
                                   (bus.clk  == target.clk  || !mask.clk)  &&
                                   (bus.atn  == target.atn  || !mask.atn) {
                                    let _ = drive.bus().hardware.lockstep_tx.as_ref().unwrap()
                                        .send(LockStepResponse::Done(bus, drive.pc()));
                                    timed_out = false;
                                    break;
                                }
                            }
                            if timed_out {
                                let _ = drive.bus().hardware.lockstep_tx.as_ref().unwrap()
                                    .send(LockStepResponse::Timeout(drive.bus().hardware.bus_state, drive.pc()));
                            }
                        }
                    }
                    continue; // Check for more lockstep commands immediately
                }
            }
            drive.cpu.tick();
        }
    });

    let mut bus = LockStepBus { 
        cmd_tx: ls_cmd_tx, 
        resp_rx: ls_resp_rx, 
        host_drives: BusSnapshot::default(),
        last_pc: 0,
    };

    // 1. Initial boot run (5 million cycles)
    println!("Booting drive...");
    bus.run(5_000_000).expect("Initial boot run failed");

    // 2. First read: power-on status (73) or already-cleared (00).
    println!("Requesting status (first)...");
    let status = iec_read_status(&mut bus)
        .expect("IEC status request timed out (first read)");
    
    println!("Drive status (first): {:?}", status);
    assert!(!status.is_empty(), "Status string was empty");

    let error_num: u8 = status[0..2].parse().expect("Invalid status number format");
    assert!(error_num == 73 || error_num == 0, "Expected status 73 or 00, got {}", error_num);

    // 3. Second read clears the message; must be 00.
    println!("Requesting status (second)...");
    let status2 = iec_read_status(&mut bus)
        .expect("IEC status request timed out (second read)");

    println!("Drive status (second): {:?}", status2);

    let error_num2: u8 = status2[0..2].parse().expect("Invalid status number format");
    assert_eq!(error_num2, 0, "Expected status 00 after clearing read, got {}", error_num2);

    running.store(false, Ordering::SeqCst);
}

/// Verifies directory load ("$").
#[test]
fn test_dir_after_mount() {
    let _ = env_logger::builder().is_test(true).try_init();
    let d64_data = make_minimal_d64();
    let disk_id = [0x31, 0x31]; // ID "11"

    let storage = MemoryStorage::new(&d64_data);
    let mut img = D64Image::new(storage);
    img.set_id(disk_id);

    // Build GCR for track 1 (half-track 2) — used during the BUMP phase.
    let mut gcr_t1 = vec![0u8; MAX_TRACK_BYTES];
    let (bit_len_t1, _) = futures::executor::block_on(
        DiskImage::read_track(&mut img, 2, &mut gcr_t1)
    ).expect("read_track t1 failed");

    // Build GCR for track 18 (half-track 36) — contains the BAM/directory.
    let mut gcr_t18 = vec![0u8; MAX_TRACK_BYTES];
    let (bit_len_t18, _) = futures::executor::block_on(
        DiskImage::read_track(&mut img, 36, &mut gcr_t18)
    ).expect("read_track t18 failed");

    // Boot the drive without a disk inserted.
    let mut drive = make_drive();
    
    // Populate the track library for ALL half-tracks from 1 to 36.
    {
        let hw = &mut drive.cpu.bus.hardware;
        // Half-tracks 1..10: serve Track 1 data
        for ht in 1..=10 {
            hw.track_library.push((ht, gcr_t1.clone(), bit_len_t1));
        }
        // Half-tracks 11..36: serve Track 18 data
        for ht in 11..=36 {
            hw.track_library.push((ht, gcr_t18.clone(), bit_len_t18));
        }
        // Initial head position for BUMP sequence
        hw.load_nearest_track(36);
        hw.active_track.disk_inserted = true;
    }

    boot_drive(&mut drive);

    // Request the directory (LOAD "$").
    let dir_result = iec_load_file(&mut drive, "$");
    
    if let Err(ref e) = dir_result {
        eprintln!("[TEST] iec_load_file failed: {}", e);
    }
    
    let dir_data = dir_result.expect("iec_load_file failed");
    println!("Raw directory data ({} bytes): {:02X?}", dir_data.len(), dir_data);

    if dir_data.is_empty() {
        let final_status = iec_read_status(&mut drive).expect("Failed to read final status");
        panic!("iec_load_file returned no data. Drive status: {}", final_status);
    }

    let dir_text = parse_directory(&dir_data);

    println!("Directory listing:\n{}", dir_text);

    assert!(
        dir_text.contains("TEST DISK"),
        "Expected disk name 'TEST DISK' in directory output, got:\n{}",
        dir_text
    );
    assert!(
        dir_text.contains("HELLO"),
        "Expected filename 'HELLO' in directory output, got:\n{}",
        dir_text
    );
}
