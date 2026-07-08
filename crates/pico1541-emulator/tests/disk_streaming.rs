use pico1541_common::magnetic_surface::{MagneticSurface, SoftwareMagneticSurface};

fn wait_for_pulse(mechanics: &mut DriveMechanics<SoftwareMagneticSurface>, hardware: &mut TestBridge, via1: &mut Via6522, via2: &mut Via6522, mut last_pulse: bool) -> (u32, bool) {
    let mut ticks = 0;
    loop {
        ticks += 1;
        let pulse = mechanics.tick_cycles(hardware, via1, via2, 1);
        if pulse && !last_pulse {
            return (ticks, pulse);
        }
        last_pulse = pulse;
        if ticks > 100 { panic!("Timeout"); }
    }
}
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
        inactive.data[0] = 0xAA;
        inactive.data[1] = 0x55;
        inactive.data[2] = 0x01;
        inactive.data[3] = 0xFE;
    }
    manager.commit_loaded_track(0, TrackLayout::Accurate, track_bytes, None);

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let surface = SoftwareMagneticSurface::new(emulator);
    let mut mechanics = DriveMechanics::new(surface);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    mechanics.surface.start_reading(0, 0);

    // Set bitrate Zone 3 (26 cycles) explicitly
    // In our implementation Zone 3 (26 cycles) is bits 5,6 = 11 (0x60)
    via2.ddrb = 0xFF; // All bits are output
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 3 (26 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);

    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)

    // PCR: CB2=111 (Manual High), CA2=111 (Manual High) -> 0b1110_1110
    via2.pcr = 0b1110_1110;

    // First byte delivered should be idx 0: 0xAA. Should take exactly 26 ticks.
    let (ticks, last_pulse) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, false);
    assert_eq!(ticks, 26);
    assert_eq!(via2.in_a, 0xAA, "Should have delivered idx 0");
    assert!(via2.ifr & 0x02 != 0, "VIA2 CA1 should have been pulsed");
    via2.ifr &= !0x02; // Clear it

    // Second byte idx 1: 0x55. Exactly 26 ticks.
    let (ticks, _) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, last_pulse);
    assert_eq!(ticks, 26);
    assert_eq!(via2.in_a, 0x55);
}

#[test]
fn test_disk_streaming_sync() {
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (manager, emulator) = buffer.split();
    let mut hardware = TestBridge::new(8, buffer);
    
    let inactive = manager.start_track_load();
    inactive[0] = 0xFF; // SYNC (First byte, will be treated as data)
    inactive[1] = 0xFF; // SYNC (Second byte, establishes SYNC)
    inactive[2] = 0x55; // DATA
    manager.commit_loaded_track(0, TrackLayout::Accurate, 3, None);

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let surface = SoftwareMagneticSurface::new(emulator);
    let mut mechanics = DriveMechanics::new(surface);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    mechanics.surface.start_reading(0, 0);
    via2.ddrb = 0xFF;
    via2.orb = (via2.orb & !0x60) | 0x60 | 0x04; // Zone 3 (26 cycles) + Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    
    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)
    via2.pcr = 0b1110_1110;

    // First byte idx 0: 0xFF
    // Due to the new SYNC logic, the FIRST 0xFF is treated as data and pulses SO!
    let mut ticks = 0;
    while !mechanics.tick_cycles(&mut hardware, &mut via1, &mut via2, 1) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for first byte");
        }
    }
    assert!(via2.in_b & 0x80 != 0, "SYNC should NOT be indicated yet");
    
    // Wait for the SO pulse to clear
    while mechanics.tick_cycles(&mut hardware, &mut via1, &mut via2, 1) {}

    // Second byte idx 1: 0xFF
    // Now last_gcr_byte == 0xFF, so it establishes SYNC!
    for i in 0..26 {
        assert!(
            !mechanics.tick_cycles(&mut hardware, &mut via1, &mut via2, 1),
            "SYNC should NOT pulse SO (cycle {})", i
        );
    }
    assert!(via2.in_b & 0x80 == 0, "SYNC should be indicated on Port B bit 7");

    // Third byte idx 2: 0x55 (Data)
    // This one IS data, so it should return true on the 26th cycle.
    let mut ticks = 0;
    while !mechanics.tick_cycles(&mut hardware, &mut via1, &mut via2, 1) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for data byte after SYNC");
        }
    }

    assert!(
        via2.in_b & 0x80 != 0,
        "SYNC should be RELEASED when data byte arrives"
    );
}

#[test]
fn test_disk_bitrate_zones() {
    let track_bytes = 8000;
    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (manager, emulator) = buffer.split();
    let mut hardware = TestBridge::new(8, buffer);
    
    {
        let inactive = manager.get_dormant_buffer_mut();
        inactive.data.fill(0x55); // Use data, not SYNC
    }
    manager.commit_loaded_track(0, TrackLayout::Accurate, track_bytes, None);

    let mut via1 = Via6522::new(); let mut via2 = Via6522::new();
    let surface = SoftwareMagneticSurface::new(emulator);
    let mut mechanics = DriveMechanics::new(surface);
    mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    mechanics.surface.start_reading(0, 0);
    via2.ddrb = 0xFF;
    via2.pcr = 0b1110_1110; // Enable SO

    // Test Zone 3: 26 cycles (bits 5,6 = 11 -> 0x60)
    via2.orb = 0x60 | 0x04; // Motor ON
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    let (ticks, lp3) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, false);
    assert_eq!(ticks, 26, "Zone 3 should take 26 cycles");

    // Test Zone 2: 28 cycles (bits 5,6 = 10 -> 0x40)
    via2.orb = 0x40 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    let (ticks, lp2) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, lp3);
    assert_eq!(ticks, 28, "Zone 2 should take 28 cycles");

    // Test Zone 1: 30 cycles (bits 5,6 = 01 -> 0x20)
    via2.orb = 0x20 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    let (ticks, lp1) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, lp2);
    assert_eq!(ticks, 30, "Zone 1 should take 30 cycles");

    // Test Zone 0: 32 cycles (bits 5,6 = 00 -> 0x00)
    via2.orb = 0x00 | 0x04;
    mechanics.update_state(via2.orb, via2.ddrb, &mut hardware);
    let (ticks, _) = wait_for_pulse(&mut mechanics, &mut hardware, &mut via1, &mut via2, lp1);
    assert_eq!(ticks, 32, "Zone 0 should take 32 cycles");
}

