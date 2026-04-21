mod common;

use common::TestBridge;
use pico1541_emulator::Emulator;

use log::info;


fn read_status(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) -> String {
    bridge.atn_start(drive);

    bridge.send_byte(drive, 0x48, false); // TALK 8
    bridge.send_byte(drive, 0x6F, false); // SEC 15
    
    // Turnaround: Switch from Talker to Listener
    bridge.atn_end(drive);
    bridge.turnaround(drive);

    // Receive all bytes until EOI
    let bytes = bridge.receive_all(drive);

    // End of communication: Host releases everything
    bridge.iec_end(drive);

    String::from_utf8_lossy(&bytes)
        .trim_end_matches('\r')
        .to_string()
}

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);
    
    assert!(!bridge.bus_data, "Drive DATA should NOT be asserted");
    assert!(!bridge.bus_clk, "Drive CLK should NOT be asserted");

    // Issue an ATN start sequence
    bridge.atn_start(&mut drive);

    assert!(bridge.bus_data, "Drive DATA should be asserted");

    // Issue an ATN stop sequence
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);

    assert!(!bridge.bus_data, "Drive DATA should NOT be asserted");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);

    // ── First status read: boot message ──────────────────────────────────────
    let status1 = read_status(&mut drive, &mut bridge);
    info!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected boot status starting with '73,', got: {:?}", status1
    );

    // ── Second status read: should clear to 00, OK ────────────────────────────
    let status2 = read_status(&mut drive, &mut bridge);
    info!("Status 2: {:?}", status2);
    assert!(
        status2.starts_with("00,"),
        "Expected '00,...' on second read, got: {:?}", status2
    );
}
