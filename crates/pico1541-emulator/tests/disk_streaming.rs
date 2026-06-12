mod common;

use common::TestBridge;
use pico1541_common::track_buffer::TrackLayout;
use pico1541_emulator::drivemechanics::DriveMechanics;
use pico1541_emulator::via6522::Via6522;

#[test]
fn test_disk_streaming_data() {
    let track_bytes = 8000;
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (manager, emulator) = buffer.split();
    let mut hardware = TestBridge::new(8, buffer);
    
    {
        let inactive = manager.get_dormant_buffer_mut();
        inactive.layout = TrackLayout::Accurate;
        inactive.length = track_bytes;
        inactive.track_id = 0; // Match DriveMechanics default
        inactive.data[0] = 0xAA;
        inactive.data[1] = 0x55;
        inactive.data[2] = 0x01;
        inactive.data[3] = 0xFE;
    }
    manager.commit_dormant_buffer();

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new(emulator);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)

    // Set bitrate Zone 3 (26 cycles) explicitly
    // In our implementation Zone 3 (26 cycles) is bits 5,6 = 11 (0x60)
    via2.ddrb = 0xFF; // All bits are output
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 3 (26 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);

    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)

    // PCR: CB2=111 (Manual High), CA2=111 (Manual High) -> 0b1110_1110
    via2.pcr = 0b1110_1110;

    // First byte delivered should be idx 0: 0xAA. Should take exactly 26 ticks.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
            panic!("Timeout waiting for idx 0");
        }
    }
    assert_eq!(ticks + 1, 26);
    assert_eq!(via2.in_a, 0xAA, "Should have delivered idx 0");
    assert!(via2.ifr & 0x02 != 0, "VIA2 CA1 should have been pulsed");
    via2.ifr &= !0x02; // Clear it

    // Second byte idx 1: 0x55. Exactly 26 ticks.
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
            panic!("Timeout waiting for idx 1");
        }
    }
    assert_eq!(ticks + 1, 26);
    assert_eq!(via2.in_a, 0x55);
}

#[test]
fn test_disk_streaming_sync() {
    let track_bytes = 8000;
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let hardware = TestBridge::new(8, buffer);
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (manager, emulator) = buffer.split();
    let mut hardware = TestBridge::new(8, buffer);
    
    {
        let inactive = manager.get_dormant_buffer_mut();
        inactive.layout = TrackLayout::Accurate;
        inactive.length = track_bytes;
        inactive.track_id = 0;
        inactive.data[0] = 0xFF; // SYNC
        inactive.data[1] = 0x55;
    }
    manager.commit_dormant_buffer();

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new(emulator);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    via2.ddrb = 0xFF;
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 3 (26 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);
    
    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)
    via2.pcr = 0b1110_1110;

    // First byte idx 0: 0xFF (SYNC)
    // Run for 26 cycles. SYNC suppresses SO, so tick() must return false.
    for i in 0..26 {
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
    // This one IS data, so it should return true on the 26th cycle.
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for data byte after SYNC");
        }
    }
    assert_eq!(ticks + 1, 26);
    assert_eq!(via2.in_a, 0x55);
    assert!(via2.in_b & 0x80 != 0, "SYNC should be cleared");
}

#[test]
fn test_disk_bitrate_zones() {
    let track_bytes = 8000;
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (manager, emulator) = buffer.split();
    let mut hardware = TestBridge::new(8, buffer);
    
    {
        let inactive = manager.get_dormant_buffer_mut();
        inactive.layout = TrackLayout::Accurate;
        inactive.length = track_bytes;
        inactive.track_id = 0;
        inactive.data.fill(0x55); // Use data, not SYNC
    }
    manager.commit_dormant_buffer();

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new(emulator);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    via2.ddrb = 0xFF;
    via2.pcr = 0b1110_1110; // Enable SO

    // Test Zone 3: 26 cycles (bits 5,6 = 11 -> 0x60)
    via2.orb = 0x60 | 0x04; // Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);
    let mut ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 3"); }
    }
    assert_eq!(ticks + 1, 26, "Zone 3 should take 26 cycles");

    // Test Zone 2: 28 cycles (bits 5,6 = 10 -> 0x40)
    via2.orb = 0x40 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 2"); }
    }
    assert_eq!(ticks + 1, 28, "Zone 2 should take 28 cycles");

    // Test Zone 1: 30 cycles (bits 5,6 = 01 -> 0x20)
    via2.orb = 0x20 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 1"); }
    }
    assert_eq!(ticks + 1, 30, "Zone 1 should take 30 cycles");

    // Test Zone 0: 32 cycles (bits 5,6 = 00 -> 0x00)
    via2.orb = 0x00 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware, 0, 0);
    ticks = 0;
    while !mechanics.tick(&mut hardware, &mut via1, &mut via2, 0) {
        ticks += 1;
        if ticks > 100 { panic!("Timeout Zone 0"); }
    }
    assert_eq!(ticks + 1, 32, "Zone 0 should take 32 cycles");
}

