mod common;

use pico1541_common::interface::DriveEvent;
use log::info;
use pico1541_common::iec_com::{IecCommander, IecError};

use pollster::block_on;

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    assert!(
        !bridge.drive_iec.get_data(),
        "Drive DATA should be released initially"
    );

    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus).open(8, 15).execute().await
    })
    .unwrap();

    assert!(
        !bridge.drive_iec.get_data()    ,
        "Drive DATA should be released after ATN end"
    );
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    // Clear boot status
    let mut status1_vec = Vec::new();
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .receive(|b| status1_vec.push(b))
            .await
    })
    .unwrap();
    let status1 = String::from_utf8_lossy(&status1_vec)
        .trim_end_matches('\r')
        .to_string();

    info!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected '73,...' on first read, got: {:?}",
        status1
    );

    let mut status2_vec = Vec::new();
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .receive(|b| status2_vec.push(b))
            .await
    })
    .unwrap();
    let status2 = String::from_utf8_lossy(&status2_vec)
        .trim_end_matches('\r')
        .to_string();

    info!("Status 2: {:?}", status2);
    assert!(
        status2.starts_with("00,"),
        "Expected '00,...' on second read, got: {:?}",
        status2
    );
}

#[test]
fn test_command_snoop() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    // 1. Send "M-R" exactly like a DIR command
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .send(b"M-R\x00\x02")
            .execute()
            .await?;

        bus.bridge.ticks(bus.drive, 100);

        Ok::<(), IecError>(())
    })
    .unwrap();

    let mut snooped_command = None;
    let mut handler = pico1541_common::snooper::IecSnoopByteHandler::new();
    for event in bridge.events {
        handler.handle(event, &mut |e| {
            if let DriveEvent::IecCommand { data, len } = e {
                snooped_command = Some(String::from_utf8_lossy(&data[..len as usize]).to_string());
            }
        });
    }

    log::info!("Snooped command: {:?}", snooped_command);
    assert_eq!(snooped_command, Some("M-R\x00\x02".to_string()));
}

#[test]
fn test_hijack_command_snoop() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    // Clear boot status
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .receive(|_| {})
            .await
    })
    .unwrap();
    //bridge.snooper = pico1541_common::snooper::IecSnooper::new(bridge.device_id, bridge.config);

    // 1. Send "CD:GAMES" exactly like a DIR command
    // We expect a Timeout because we "yank the cable" (hijack) as soon as "CD:" is detected.
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        let res = IecCommander::new(&mut bus)
            .open(8, 0xFF)
            .send(b"CD:GAMES")
            .execute()
            .await;
        
        // We now expect the command to finish gracefully before the hijack takes effect
        assert!(res.is_ok(), "Expected command to finish gracefully");
        
        Ok::<(), IecError>(())
    })
    .unwrap();

    // (Rest of the test remains same...)
    let mut handler = pico1541_common::snooper::IecSnoopByteHandler::new();
    for event in &bridge.events {
        handler.handle(*event, &mut |_| {});
    }

    assert!(handler.is_hijacked(), "Command should be hijacked");
    assert_eq!(handler.get_command(), Some("CD:GAMES"));

    // 3. Check drive status - should be 00, OK because the snooper hijacks CD:
    // We must ensure the hijack is CLEARED before checking status
    bridge.reset(); // Reset the bridge to clear hijacked state for status check
    
    let mut status_vec = Vec::new();
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .receive(|b| status_vec.push(b))
            .await
    })
    .unwrap();
    let status = String::from_utf8_lossy(&status_vec)
        .trim_end_matches('\r')
        .to_string();

    info!("Status after snoop: {:?}", status);
    assert!(
        status.starts_with("00,"),
        "Expected 00, OK, got: {:?}",
        status
    );
}

#[test]
fn test_iec_load_command() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 15)
            .send(b"I")
            .execute()
            .await
    })
    .unwrap();
}
