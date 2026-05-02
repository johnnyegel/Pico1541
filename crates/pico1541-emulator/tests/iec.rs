mod common;

use log::info;
use pico1541_emulator::interface::DriveEvent;
use pico1541_common::iec_com::{IecError, IecCommander};
use pollster::block_on;

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);
    
    assert!(!bridge.drive_data, "Drive DATA should be released initially");
    
    block_on(async {
        let mut bus = common::IecTestBridge { drive: &mut drive, bridge: &mut bridge };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .execute()
            .await
    }).unwrap();

    assert!(!bridge.drive_data, "Drive DATA should be released after ATN end");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);
    
    // Clear boot status
    let status1 = common::read_status(&mut drive, &mut bridge);
    info!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected '73,...' on first read, got: {:?}", status1
    );

    let status2 = common::read_status(&mut drive, &mut bridge);
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
    let _ = common::read_status(&mut drive, &mut bridge);
    bridge.snooper = pico1541_emulator::snooper::IecSnooper::new(bridge.device_id);

    // 1. Send "CD:GAMES" exactly like a DIR command
    block_on(async {
        let mut bus = common::IecTestBridge { drive: &mut drive, bridge: &mut bridge };
        IecCommander::new(&mut bus)
            .open(8, 0xFF)
            .send(b"CD:GAMES")
            .execute()
            .await?;

        bus.bridge.ticks(bus.drive, 100);

        // Terminate
        IecCommander::new(&mut bus)
            .open(0, 0) // UNLISTEN is 0x3F, but IecCommander doesn't support raw commands easily yet.
            // Wait, IecCommander::open(device, secondary) sends IEC_LISTEN | device then 0x60 | secondary.
            // UNLISTEN is just a single byte 0x3F.
            // My IecCommander doesn't have a way to send UNLISTEN directly without a device.
            // Actually, UNLISTEN is sent by IecCommander::execute() at the end of a LISTEN transaction.
            
            // So if I want to send UNLISTEN, I just need to use a normal open/send sequence.
            .open(8, 0x1F) // 0x20 | 8 then 0x7F? No.
            
            .execute()
            .await?;

        Ok::<(), IecError>(())
    }).unwrap();
    
    // (Rest of the test remains same...)
    let cmd_event = bridge.events.iter().find(|e| matches!(e, DriveEvent::IecCommand { .. }));
    assert!(cmd_event.is_some(), "Should have received IecCommand event");
    
    if let Some(DriveEvent::IecCommand { data, len }) = cmd_event {
        let cmd_str = core::str::from_utf8(&data[..*len as usize]).unwrap();
        info!("Snooped command: {}", cmd_str);
        assert_eq!(cmd_str, "CD:GAMES");
    }

    // 3. Check drive status - should be 00, OK because the snooper hijacks CD:
    let status = common::read_status(&mut drive, &mut bridge);
    info!("Status after snoop: {:?}", status);
    assert!(status.starts_with("00,"), "Expected 00, OK, got: {:?}", status);
}
