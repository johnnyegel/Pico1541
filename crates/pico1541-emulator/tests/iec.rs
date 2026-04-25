mod common;

use log::info;
use pico1541_emulator::interface::{DriveEvent, HostInterface};

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);
    
    assert!(!bridge.bus_data, "Drive DATA should be released initially");
    
    bridge.atn_start(&mut drive);
    assert!(bridge.bus_data, "Drive DATA should be pulled after ATN start");
    
    bridge.atn_end(&mut drive);
    bridge.ticks(&mut drive, common::T_PHASE * 2);
    assert!(!bridge.bus_data, "Drive DATA should be released after ATN end");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);
    
    let status1 = bridge.read_status(&mut drive);
    info!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected '73,...' on first read, got: {:?}", status1
    );

    let status2 = bridge.read_status(&mut drive);
    info!("Status 2: {:?}", status2);
    assert!(
        status2.starts_with("00,"),
        "Expected '00,...' on second read, got: {:?}", status2
    );
}

#[test]
fn test_manager_command_snoop() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);
    
    // Clear boot status
    let _ = bridge.read_status(&mut drive);
    bridge.snooper = pico1541_emulator::snooper::IecSnooper::new();

    // 1. Send "CD:GAMES" exactly like a DIR command
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x28, false); // LISTEN 8
    bridge.send_byte(&mut drive, 0xFF, false); // OPEN 15
    bridge.atn_end(&mut drive);

    // Send "CD:GAMES" as data (with ATN deasserted)
    bridge.send_byte(&mut drive, b'C', false);
    bridge.send_byte(&mut drive, b'D', false);
    bridge.send_byte(&mut drive, b':', false);
    bridge.send_byte(&mut drive, b'G', false);
    bridge.send_byte(&mut drive, b'A', false);
    bridge.send_byte(&mut drive, b'M', false);
    bridge.send_byte(&mut drive, b'E', false);
    bridge.send_byte(&mut drive, b'S', false); // No EOI

    // Wait for the last byte ACK to finish completely
    bridge.ticks(&mut drive, common::T_PHASE * 2);

    // Terminate
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); // UNLISTEN
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);

    // 2. Check that IecCommand event was received by the snooper
    let cmd_event = bridge.events.iter().find(|e| matches!(e, DriveEvent::IecCommand { .. }));
    assert!(cmd_event.is_some(), "Should have received IecCommand event");
    
    if let Some(DriveEvent::IecCommand { data, len }) = cmd_event {
        let cmd_str = core::str::from_utf8(&data[..*len as usize]).unwrap();
        info!("Snooped command: {}", cmd_str);
        assert_eq!(cmd_str, "CD:GAMES");
    }

    // 3. Check drive status - should be 00, OK because the snooper hijacks CD:
    let status = bridge.read_status(&mut drive);
    info!("Status after snoop: {:?}", status);
    assert!(status.starts_with("00,"), "Expected 00, OK, got: {:?}", status);
}

#[test]
fn test_manager_command_hijack() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);

    // Clear boot status
    let _ = bridge.read_status(&mut drive);
    bridge.snooper = pico1541_emulator::snooper::IecSnooper::new();

    // 1. Send "CD:GAMES" on channel 15
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x28, false); // LISTEN 8
    bridge.send_byte(&mut drive, 0xFF, false); // OPEN 15
    bridge.atn_end(&mut drive);

    // Send "CD:GAMES" as data. 
    // Hijack should kick in after "CD:".
    bridge.send_byte(&mut drive, b'C', false);
    bridge.send_byte(&mut drive, b'D', false);
    bridge.send_byte(&mut drive, b':', false);
    
    // Check if bridge is now hijacked
    assert!(bridge.is_hijacked(), "Bridge should be hijacked after CD:");

    bridge.send_byte(&mut drive, b'G', false);
    bridge.send_byte(&mut drive, b'A', false);
    bridge.send_byte(&mut drive, b'M', false);
    bridge.send_byte(&mut drive, b'E', false);
    bridge.send_byte(&mut drive, b'S', false); // No EOI

    // Wait for the last byte ACK to finish completely
    bridge.ticks(&mut drive, common::T_PHASE * 2);

    // Terminate
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); // UNLISTEN
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);

    // 2. Check that IecCommand event was received by the snooper
    let cmd_event = bridge.events.iter().find(|e| matches!(e, DriveEvent::IecCommand { .. }));
    assert!(cmd_event.is_some(), "Should have received IecCommand event");
    
    if let Some(DriveEvent::IecCommand { data, len }) = cmd_event {
        let cmd_str = core::str::from_utf8(&data[..*len as usize]).unwrap();
        info!("Hijacked command: {}", cmd_str);
        assert_eq!(cmd_str, "CD:GAMES");
    }

    // Wait for the ROM to idle
    bridge.ticks(&mut drive, 100_000);

    // 3. Check drive status - should be 00, OK because we hijacked and discarded
    let status = bridge.read_status(&mut drive);
    info!("Status after hijack: {:?}", status);
    assert!(status.starts_with("00,"), "Expected 00, OK, got: {:?}", status);
}
