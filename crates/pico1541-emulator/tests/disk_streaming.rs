mod common;

use common::TestBridge;
use pico1541_emulator::via6522::Via6522;
use pico1541_emulator::drivemechanics::DriveMechanics;

#[test]
fn test_disk_streaming_data() {
    let mut hardware = TestBridge::new();
    hardware.track.disk_inserted = true;
    hardware.track.bit_length = 8000 * 8;
    
    // Set some test GCR data
    hardware.track.gcr_data[0] = 0xAA;
    hardware.track.gcr_data[1] = 0x55;
    hardware.track.gcr_data[2] = 0x01;
    hardware.track.gcr_data[3] = 0xFE;

    let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();

    // Set bitrate Zone 0 (32 cycles) explicitly
    via2.orb = (via2.orb & !0x60) | 0x00;

    // Set READ mode (PB4=1)
    via2.orb |= 0x10;

    // Turn Motor ON and wait for spin-up
    via2.orb |= 0x04;
    for _ in 0..250_000 {
        mechanics.tick(&mut hardware, &mut via2);
    }
    assert!(mechanics.motor_spun_up);
    
    // Force reset for exact measurement starting from index 0
    mechanics.sub_cycle_counter = 0;
    let track_bytes = (hardware.track.bit_length / 8) as usize;
    mechanics.current_byte_idx = track_bytes - 1;

    // First byte delivered should be idx 0: 0xAA. Should take exactly 32 ticks.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout"); }
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0xAA, "Should have delivered idx 0");
    assert!(via2.ifr & 0x02 != 0, "VIA2 CA1 should have been pulsed");
    via2.ifr &= !0x02; // Clear it

    // Second byte idx 1: 0x55. Exactly 32 ticks.
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0x55);
}

#[test]
fn test_disk_streaming_sync() {
    let mut hardware = TestBridge::new();
    hardware.track.disk_inserted = true;
    hardware.track.bit_length = 8000 * 8;
    
    hardware.track.gcr_data[0] = 0xFF; // SYNC
    hardware.track.gcr_data[1] = 0x55;

    let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();
    via2.orb = (via2.orb & !0x60) | 0x00; // Zone 0
    via2.orb |= 0x10; // READ mode (PB4=1)

    via2.orb |= 0x04;
    for _ in 0..250_000 { mechanics.tick(&mut hardware, &mut via2); }
    
    mechanics.sub_cycle_counter = 0;
    let track_bytes = (hardware.track.bit_length / 8) as usize;
    mechanics.current_byte_idx = track_bytes - 1;

    // First byte idx 0: 0xFF (SYNC)
    for _ in 0..32 {
        assert!(!mechanics.tick(&mut hardware, &mut via2), "SYNC should not pulse SO");
    }
    
    // SYNC should set VIA2 Port B bit 7
    assert!(via2.in_b & 0x80 != 0, "SYNC should be indicated on Port B bit 7");
    assert_eq!(via2.in_a, 0xFF, "Port A should be updated even for SYNC");
    
    // Next byte idx 1: 0x55 (Data)
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0x55);
    assert!(via2.in_b & 0x80 == 0, "SYNC should be cleared");
}

#[test]
fn test_disk_bitrate_zones() {
    let mut hardware = TestBridge::new();
    hardware.track.disk_inserted = true;
    hardware.track.bit_length = 8000 * 8;
    hardware.track.gcr_data[0] = 0xAA;
    hardware.track.gcr_data[1] = 0x55;

    let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();

    via2.orb |= 0x04; // Motor ON
    for _ in 0..250_000 { mechanics.tick(&mut hardware, &mut via2); }

    // Test Zone 3: 26 cycles (bits 5,6 = 11 -> 0x60)
    via2.orb = (via2.orb & !0x60) | 0x60;
    // Toggling bits 5,6 should have reset sub_cycle_counter to 0.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 26, "Zone 3 should take 26 cycles");

    // Test Zone 2: 28 cycles (bits 5,6 = 10 -> 0x40)
    via2.orb = (via2.orb & !0x60) | 0x40;
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 28, "Zone 2 should take 28 cycles");

    // Test Zone 1: 30 cycles (bits 5,6 = 01 -> 0x20)
    via2.orb = (via2.orb & !0x60) | 0x20;
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 30, "Zone 1 should take 30 cycles");

    // Test Zone 0: 32 cycles (bits 5,6 = 00 -> 0x00)
    via2.orb = (via2.orb & !0x60) | 0x00;
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via2) {
        ticks += 1;
    }
    assert_eq!(ticks + 1, 32, "Zone 0 should take 32 cycles");
}

#[test]
fn test_disk_write_streaming() {
    let mut hardware = TestBridge::new();
    hardware.track.disk_inserted = true;
    hardware.track.bit_length = 8000 * 8;
    
    let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();

    via2.orb |= 0x04; // Motor ON
    for _ in 0..250_000 { mechanics.tick(&mut hardware, &mut via2); }

    // Set Write mode: PB4=0
    via2.orb &= !0x10;
    // Data to write
    via2.ora = 0x5A;

    // Zone 0: 32 cycles
    let mut delivered = false;
    for _ in 0..32 {
        if mechanics.tick(&mut hardware, &mut via2) {
            delivered = true;
        }
    }
    assert!(delivered);
    
    // Check if written to idx 1
    assert_eq!(hardware.track.gcr_data[1], 0x5A);
    assert!(hardware.track.is_dirty);
}
