use pico1541_emu::Emulator;
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::HardwareBridge;
use pico1541_emu::Command;

#[test]
fn test_drive_boot() {
    // Load the actual ROM file from the roms directory.
    // Path is relative to the test file location in crates/pico1541-emu/tests/
    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(rom);

    // Set up hardware bridge with static lifetimes for the test.
    static TRACK: ActiveTrack = ActiveTrack::new();
    static mut COMMAND_QUEUE: heapless::spsc::Queue<Command, 16> = heapless::spsc::Queue::new();
    
    let (cmd_tx, _cmd_rx) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };
    let bridge = HardwareBridge::new(&TRACK, cmd_tx);
    
    // Initialize emulator
    let mut drive = Emulator::new(rom_array, bridge);
    drive.reset();
    
    let initial_pc = drive.pc();
    // Reset vector for 1541 usually points to $EAA0
    assert!(initial_pc >= 0xC000);
    
    // Step for a few thousand instructions
    for _ in 0..10000 {
        drive.step();
    }
    
    // Verify that the Program Counter has moved
    assert_ne!(drive.pc(), initial_pc);
    
    // After some execution, it should still be in a valid ROM range
    assert!(drive.pc() >= 0xC000);
}
