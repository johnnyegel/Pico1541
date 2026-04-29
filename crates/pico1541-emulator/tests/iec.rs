mod common;

use log::info;
use pico1541_emulator::interface::DriveEvent;
use pico1541_common::iec::{IEC_LISTEN, IEC_UNLISTEN};
use pollster::block_on;

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);
    
    assert!(!bridge.last_drv_data, "Drive DATA should be released initially");
    
    let com = common::iec_com(&mut drive, &mut bridge);
    let com = block_on(com.atn_assert()).unwrap();
    
    let _ = block_on(com.atn_release()).unwrap();
    assert!(!bridge.last_drv_data, "Drive DATA should be released after ATN end");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, 9, None);
    
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
        let com = common::iec_com(&mut drive, &mut bridge);
        let mut com = com.atn_assert().await.unwrap()
            .send(IEC_LISTEN | 8).await.unwrap()
            .send(0xFF).await.unwrap()
            .atn_release().await.unwrap();

        for &b in b"CD:GAMES" {
            com = com.send(b).await.unwrap();
        }

        // Wait for the last byte ACK to finish completely
        let bus = com.done();
        bus.bridge.ticks(bus.drive, common::T_PHASE * 2);

        // Terminate
        let com = common::iec_com(bus.drive, bus.bridge);
        let com = com.atn_assert().await.unwrap()
            .send(IEC_UNLISTEN).await.unwrap()
            .atn_release().await.unwrap();

        let _ = com.done();
    });
    
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
