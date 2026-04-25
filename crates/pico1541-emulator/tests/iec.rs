mod common;

use log::info;
use pico1541_emulator::interface::{DriveEvent, HostInterface};
use racc6502::bus::Bus;

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);
    
    assert!(!bridge.bus_data, "Drive DATA should be released initially");
    
    bridge.atn_start(&mut drive);
    assert!(bridge.bus_data, "Drive DATA should be pulled after ATN start");
    
    bridge.atn_end(&mut drive);
    bridge.ticks(&mut drive, common::T_PHASE * 2);
    assert!(!bridge.bus_data, "Drive DATA should be released after ATN end");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, 9, None);
    
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
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);
    
    // Clear boot status
    let _ = bridge.read_status(&mut drive);
    bridge.snooper = pico1541_emulator::snooper::IecSnooper::new(bridge.device_id);

    // 1. Send "CD:GAMES" exactly like a DIR command
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x20 | bridge.device_id, false); // LISTEN device
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
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    // Clear boot status
    let _ = bridge.read_status(&mut drive);
    bridge.snooper = pico1541_emulator::snooper::IecSnooper::new(bridge.device_id);

    // 1. Send "CD:GAMES" on channel 15
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x20 | bridge.device_id, false); // LISTEN device
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

#[test]
fn test_soft_id_change_hijack() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    // 1. Initially should be 0x28 / 0x48
    assert_eq!(bridge.snooper.listen_addr, 0x28);
    assert_eq!(bridge.snooper.talk_addr, 0x48);

    // 2. Soft-change to device 9 in RAM
    drive.bus_mut().write(0x0077, 0x29, &mut bridge); // LISTEN 9
    drive.bus_mut().write(0x0078, 0x49, &mut bridge); // TALK 9
    bridge.ticks(&mut drive, 10);
    assert_eq!(bridge.snooper.listen_addr, 0x29);
    assert_eq!(bridge.snooper.talk_addr, 0x49);

    // 3. Soft-change to device 10 in RAM
    drive.bus_mut().write(0x0077, 0x2A, &mut bridge); // LISTEN 10
    drive.bus_mut().write(0x0078, 0x4A, &mut bridge); // TALK 10
    bridge.ticks(&mut drive, 10);
    assert_eq!(bridge.snooper.listen_addr, 0x2A);
    assert_eq!(bridge.snooper.talk_addr, 0x4A);

    // 4. Change back to 8 and verify we can still hijack
    drive.bus_mut().write(0x0077, 0x28, &mut bridge);
    drive.bus_mut().write(0x0078, 0x48, &mut bridge);
    bridge.ticks(&mut drive, 10);

    // Clear boot status
    let _ = bridge.read_status(&mut drive);
    bridge.snooper.discard_request = false;
    bridge.events.clear();

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x28, false); // LISTEN 8
    bridge.send_byte(&mut drive, 0xFF, false); // OPEN 15
    bridge.atn_end(&mut drive);

    bridge.send_byte(&mut drive, b'C', false);
    bridge.send_byte(&mut drive, b'D', false);
    bridge.send_byte(&mut drive, b':', false);
    bridge.send_byte(&mut drive, b'O', false);
    bridge.send_byte(&mut drive, b'K', true);

    bridge.ticks(&mut drive, common::T_PHASE * 2);

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); // UNLISTEN
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);

    let cmd_event = bridge.events.iter().find(|e| matches!(e, DriveEvent::IecCommand { .. }));
    assert!(cmd_event.is_some(), "Should have hijacked CD: after changing address back and forth");
}
