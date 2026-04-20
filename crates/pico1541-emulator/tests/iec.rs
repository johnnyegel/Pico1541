mod common;

use common::TestBridge;
use pico1541_emulator::Emulator;

const ROM_BYTES: &[u8] = include_bytes!("../../../roms/1541-II.251968-03.bin");

fn setup_drive() -> (Emulator<TestBridge>, TestBridge) {
    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);

    let mut drive = Emulator::<TestBridge>::new(rom);
    let mut bridge = TestBridge::new();

    reset_drive(&mut drive, &mut bridge);

    (drive, bridge)
}

fn reset_drive(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) {
    drive.reset();
    bridge.reset();

    // Boot: Wait for idle loop
    for _ in 0..5_000_000 {
        drive.tick(bridge);
        if drive.pc() >= common::IDLE_LOW && drive.pc() <= common::IDLE_HIGH {
            return;
        }
    }

    panic!("Drive did not reach idle loop after reset (PC=${:04X})", drive.pc());
}


fn read_status(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) -> String {
    bridge.atn_start(drive);

    bridge.send_byte(drive, 0x48, false);
    bridge.send_byte(drive, 0x6F, true);
    
    bridge.set_atn(false);
    bridge.set_clk(false);
    bridge.set_data(true);
    
    bridge.ticks(drive, 100);

    let bytes = bridge.receive_all(drive);

    bridge.set_atn(false);
    bridge.set_clk(false);
    bridge.set_data(false);
     bridge.ticks(drive, 100);

    String::from_utf8_lossy(&bytes)
        .trim_end_matches('\r')
        .to_string()
}

#[test]
fn test_iec_atn_handshake() {
    let (mut drive, mut bridge) = setup_drive();
    
    assert!(!bridge.bus_data, "Drive DATA should NOT be asserted");
    assert!(!bridge.bus_clk, "Drive CLK should NOT be asserted");

    // Issue an ATN start sequence
    bridge.atn_start(&mut drive);

    assert!(bridge.bus_data, "Drive DATA should be asserted");

    // Host release the clock
    bridge.set_clk(false);

    // Drive ack by releasing data
    bridge.wait_bus_data(&mut drive, false);
    assert!(!bridge.bus_data, "Drive DATA should NOT be asserted");
}

#[test]
fn test_iec_read_status() {
    let (mut drive, mut bridge) = setup_drive();

    // ── First status read: boot message ──────────────────────────────────────
    let status1 = read_status(&mut drive, &mut bridge);
    println!("Status 1: {:?}", status1);
    assert!(
        status1.starts_with("73,"),
        "Expected boot status starting with '73,', got: {:?}", status1
    );

    // ── Second status read: should clear to 00, OK ────────────────────────────
    let status2 = read_status(&mut drive, &mut bridge);
    println!("Status 2: {:?}", status2);
    assert!(
        status2.starts_with("00,"),
        "Expected '00,...' on second read, got: {:?}", status2
    );
}
