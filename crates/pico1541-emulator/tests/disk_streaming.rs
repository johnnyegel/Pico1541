#![allow(dead_code)]
use pico1541_common::magnetic_surface::{SoftwareMagneticSurface};

fn wait_for_pulse(bus: &mut pico1541_emulator::systembus::SystemBus<TestBridge, SoftwareMagneticSurface>, mut last_pulse: bool) -> (u32, bool) {
    let mut ticks = 0;
    loop {
        ticks += 1;
        bus.tick_cycles(1);
        bus.poll_mechanics(1, ticks);
        let pulse = bus.so_pending();
        if pulse && !last_pulse {
            return (ticks, pulse);
        }
        last_pulse = pulse;
        if ticks > 100 { panic!("Timeout. via2.orb={:02X}, ddrb={:02X}, pcr={:02X}, is_motor_on={}", bus.via2.ora(), bus.via2.ora(), bus.via2.pcr(), (bus.via2.ora() & 0x04) != 0); }
    }
}
mod common;

static DUMMY_ROM: [u8; 16384] = [0; 16384];

fn get_system_bus(surface: SoftwareMagneticSurface, handle: pico1541_common::track_buffer::EmulatorHandle) -> pico1541_emulator::systembus::SystemBus<TestBridge, SoftwareMagneticSurface> {
    let mechanics = pico1541_emulator::drivemechanics::DriveMechanics::new(surface, handle);
    pico1541_emulator::systembus::SystemBus::new(&DUMMY_ROM, Box::leak(Box::new([0u8; 32768])), 8, mechanics)
}

use common::TestBridge;
use pico1541_common::track_buffer::TrackLayout;

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
    manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(0, pico1541_common::track_buffer::default_speed_zone(0)), TrackLayout::Accurate, track_bytes);

    let surface = SoftwareMagneticSurface::new();
    let mut bus = get_system_bus(surface, emulator);
    bus.mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    
    bus.mechanics.force_state_for_testing(0, 3, 0);

    // Set bitrate Zone 3 (26 cycles) explicitly
    // In our implementation Zone 3 (26 cycles) is bits 5,6 = 11 (0x60)
    bus.via2.write(0x02, 0xFF); // All bits are output
    bus.via2.write(0x00, (bus.via2.ora() & !0x60) | 0x60 | 0x04); // Zone 3 (26 cycles), Motor ON
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);

    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)

    // PCR: CB2=111 (Manual High), CA2=111 (Manual High) -> 0b1110_1110
    bus.via2.write(0x0C, 0b1110_1110);
    // Enable CA1 interrupt in IER
    bus.via2.write(0x0E, 0x82);

    // First byte delivered should be idx 0: 0xAA. Should take exactly 26 ticks.
    let (ticks, last_pulse) = wait_for_pulse(&mut bus, false);
    assert_eq!(ticks, 26);
    assert_eq!(bus.via2.pa_in(), 0xAA, "Should have delivered idx 0");
    assert!(bus.via2.has_irq(), "VIA2 CA1 should have been pulsed");
    bus.via2.write(0x0D, 0x02); // Clear it

    // Second byte idx 1: 0x55. Exactly 26 ticks.
    let (ticks, _) = wait_for_pulse(&mut bus, last_pulse);
    assert_eq!(ticks, 26);
    assert_eq!(bus.via2.pa_in(), 0x55);
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
    manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(0, pico1541_common::track_buffer::default_speed_zone(0)), TrackLayout::Accurate, 3);

    let surface = SoftwareMagneticSurface::new();
    let mut bus = get_system_bus(surface, emulator);
    bus.mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    
    bus.mechanics.force_state_for_testing(0, 3, 0);
    bus.via2.write(0x02, 0xFF);
    bus.via2.write(0x00, (bus.via2.ora() & !0x60) | 0x60 | 0x04); // Zone 3 (26 cycles), Motor ON
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);
    
    // Set READ mode (CB2 out = true) AND enable SO (CA2 out = true)
    bus.via2.write(0x0C, 0b1110_1110);

    // First byte idx 0: 0xFF
    // Due to the new SYNC logic, the FIRST 0xFF is treated as data and pulses SO!
    let mut ticks = 0;
    while !({ bus.tick_cycles(1); bus.poll_mechanics(1, 0); bus.so_pending() }) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for first byte");
        }
    }
    assert!(bus.via2.pb_in() & 0x80 != 0, "SYNC should NOT be indicated yet (active low)");
    
    // Wait for the SO pulse to clear
    while { bus.tick_cycles(1); bus.poll_mechanics(1, 0); bus.so_pending() } {}

    // Second byte idx 1: 0xFF
    // Now last_gcr_byte == 0xFF, so it establishes SYNC!
    for i in 0..26 {
        assert!(
            !( { bus.tick_cycles(1); bus.poll_mechanics(1, 0); bus.so_pending() } ),
            "SYNC should NOT pulse SO (cycle {})", i
        );
    }
    assert!(bus.via2.pb_in() & 0x80 == 0, "SYNC should be indicated on Port B bit 7");

    // Third byte idx 2: 0x55 (Data)
    // This one IS data, so it should return true on the 26th cycle.
    let mut ticks = 0;
    while !({ bus.tick_cycles(1); bus.poll_mechanics(1, 0); bus.so_pending() }) {
        ticks += 1;
        if ticks > 100 {
             panic!("Timeout waiting for data byte after SYNC");
        }
    }

    assert!(
        bus.via2.pb_in() & 0x80 != 0,
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
    manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(0, pico1541_common::track_buffer::default_speed_zone(0)), TrackLayout::Accurate, track_bytes);

    let surface = SoftwareMagneticSurface::new();
    let mut bus = get_system_bus(surface, emulator);
    bus.mechanics.halftrack_head_pos = 0; // Align head with Track 1 (half-track 0)
    
    bus.mechanics.force_state_for_testing(0, 3, 0);
    bus.via2.write(0x02, 0xFF);
    bus.via2.write(0x0C, 0b1110_1110); // Enable SO

    // Test Zone 3: 26 cycles (bits 5,6 = 11 -> 0x60)
    bus.via2.write(0x00, 0x60 | 0x04); // Motor ON
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);
    let (ticks, lp3) = wait_for_pulse(&mut bus, false);
    assert_eq!(ticks, 26, "Zone 3 should take 26 cycles");

    // Test Zone 2: 28 cycles (bits 5,6 = 10 -> 0x40)
    bus.via2.write(0x00, 0x40 | 0x04);
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);
    let (ticks, lp2) = wait_for_pulse(&mut bus, lp3);
    assert_eq!(ticks, 28, "Zone 2 should take 28 cycles");

    // Test Zone 1: 30 cycles (bits 5,6 = 01 -> 0x20)
    bus.via2.write(0x00, 0x20 | 0x04);
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);
    let (ticks, lp1) = wait_for_pulse(&mut bus, lp2);
    assert_eq!(ticks, 30, "Zone 1 should take 30 cycles");

    // Test Zone 0: 32 cycles (bits 5,6 = 00 -> 0x00)
    bus.via2.write(0x00, 0x00 | 0x04);
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hardware, 0);
    let (ticks, _) = wait_for_pulse(&mut bus, lp1);
    assert_eq!(ticks, 32, "Zone 0 should take 32 cycles");
}

