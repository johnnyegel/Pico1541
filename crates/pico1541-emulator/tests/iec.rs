mod common;

use log::info;

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
    let status1 = bridge.read_status(&mut drive);
    info!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected boot status starting with '73,', got: {:?}", status1
    );

    // ── Second status read: should clear to 00, OK ────────────────────────────
    let status2 = bridge.read_status(&mut drive);
    info!("Status 2: {:?}", status2);
    assert!(
        status2.starts_with("00,"),
        "Expected '00,...' on second read, got: {:?}", status2
    );
}
