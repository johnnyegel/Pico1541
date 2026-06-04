mod common;

use common::TestBridge;
use pico1541_common::formats::TrackLayout;
use pico1541_emulator::drivemechanics::DriveMechanics;
use pico1541_emulator::via6522::Via6522;

#[test]
fn test_disk_streaming_data() {
    let track_bytes = 8000;
    let mut hardware = TestBridge::new(8);
    hardware.track.layout = TrackLayout::Accurate(track_bytes);
    hardware.track.half_track = 0; // Match DriveMechanics default

    // Set some test GCR data
    hardware.track.gcr_data[0] = 0xAA;
    hardware.track.gcr_data[1] = 0x55;
    hardware.track.gcr_data[2] = 0x01;
    hardware.track.gcr_data[3] = 0xFE;

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)

    // Set bitrate Zone 0 (32 cycles) explicitly
    // In our implementation Zone 0 (32 cycles) is bits 5,6 = 11 (0x60)
    via2.ddrb = 0xFF; // All bits are output
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 0 (32 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);

    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)

    // PCR: CB2=111 (Manual High), CA2=111 (Manual High) -> 0b1110_1110
    via2.pcr = 0b1110_1110;

    // First byte delivered should be idx 0: 0xAA. Should take exactly 32 ticks.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
            panic!("Timeout waiting for idx 0");
        }
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0xAA, "Should have delivered idx 0");
    assert!(via2.ifr & 0x02 != 0, "VIA2 CA1 should have been pulsed");
    via2.ifr &= !0x02; // Clear it

    // Second byte idx 1: 0x55. Exactly 32 ticks.
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
            panic!("Timeout waiting for idx 1");
        }
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0x55);
}

#[test]
fn test_disk_streaming_sync() {
    let track_bytes = 8000;
    let mut hardware = TestBridge::new(8);
    hardware.track.layout = TrackLayout::Accurate(track_bytes);
    hardware.track.half_track = 0;

    hardware.track.gcr_data[0] = 0xFF; // SYNC
    hardware.track.gcr_data[1] = 0x55;

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    via2.ddrb = 0xFF;
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 0 (32 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    
    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)
    via2.pcr = 0b1110_1110;

    // First byte idx 0: 0xFF (SYNC)
    // Run for 32 cycles. SYNC suppresses SO, so tick() must return false.
    for i in 0..32 {
        assert!(
            !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0),
            "SYNC should NOT pulse SO (cycle {})", i
        );
    }

    // SYNC should set VIA2 Port B bit 7 to LOW
    assert!(
        via2.in_b & 0x80 == 0,
        "SYNC should be indicated on Port B bit 7 (Active LOW)"
    );
    assert_eq!(via2.in_a, 0xFF, "Port A should be updated even for SYNC");

    // Next byte idx 1: 0x55 (Data)
    // This one IS data, so it should return true on the 32nd cycle.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for data byte after SYNC");
        }
    }
    assert_eq!(ticks + 1, 32);
    assert_eq!(via2.in_a, 0x55);
    assert!(via2.in_b & 0x80 != 0, "SYNC should be cleared");
}

#[test]
fn test_disk_bitrate_zones() {
    let track_bytes = 8000;
    let mut hardware = TestBridge::new(8);
    hardware.track.layout = TrackLayout::Accurate(track_bytes);
    hardware.track.half_track = 0;
    hardware.track.gcr_data.fill(0x55); // Use data, not SYNC

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    via2.ddrb = 0xFF;
    via2.pcr = 0b1110_1110; // Enable SO

    // Test Zone 0: 32 cycles (bits 5,6 = 11 -> 0x60)
    via2.orb = 0x60 | 0x04; // Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 0"); }
    }
    assert_eq!(ticks + 1, 32, "Zone 0 should take 32 cycles");

    // Test Zone 1: 30 cycles (bits 5,6 = 10 -> 0x40)
    via2.orb = 0x40 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 1"); }
    }
    assert_eq!(ticks + 1, 30, "Zone 1 should take 30 cycles");

    // Test Zone 2: 28 cycles (bits 5,6 = 01 -> 0x20)
    via2.orb = 0x20 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 2"); }
    }
    assert_eq!(ticks + 1, 28, "Zone 2 should take 28 cycles");

    // Test Zone 3: 26 cycles (bits 5,6 = 00 -> 0x00)
    via2.orb = 0x00 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 3"); }
    }
    assert_eq!(ticks + 1, 26, "Zone 3 should take 26 cycles");
}

#[test]
fn test_disk_write_streaming() {
    let track_bytes = 8000;
    let mut hardware = TestBridge::new(8);
    hardware.track.layout = TrackLayout::Accurate(track_bytes);
    hardware.track.half_track = 0;

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)

    via2.ddrb = 0xFF;
    via2.orb |= 0x04; // Motor ON
    via2.orb |= 0x60; // Zone 0 (32 cycles)
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);

    // Set Write mode: CB2 out = false (bits 7-5 = 110)
    // AND enable SO: CA2 out = true (bits 3-1 = 111)
    via2.pcr = 0b1100_1110;
    
    // Data to write
    via2.ora = 0x5A;

    // First byte delivered should go to idx 0
    let mut delivered = false;
    for _ in 0..32 {
        if mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
            delivered = true;
        }
    }
    assert!(delivered);

    // Check if written to idx 0
    assert_eq!(hardware.track.gcr_data[0], 0x5A);
    assert!(hardware.track.is_dirty);
}
