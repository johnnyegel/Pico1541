use pico1541_emu::Emulator;
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::HardwareBridge;
use pico1541_emu::Command;
use core::sync::atomic::Ordering;

fn init_logger() {
    let _ = env_logger::builder()
        .filter_module("mos6502", log::LevelFilter::Warn)
        .filter_module("pico1541_emu", log::LevelFilter::Info)
        .is_test(true)
        .try_init();
}

fn get_rom() -> [u8; 16384] {
    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(rom);
    rom_array
}

#[test]
fn test_drive_boot() {
    init_logger();
    let rom_array = get_rom();

    static TRACK: ActiveTrack = ActiveTrack::new();
    static mut COMMAND_QUEUE: heapless::spsc::Queue<Command, 16> = heapless::spsc::Queue::new();
    
    let (cmd_tx, _cmd_rx) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };
    let bridge = HardwareBridge::new(&TRACK, cmd_tx);
    
    let mut drive = Emulator::new(rom_array, bridge);
    drive.reset();
    
    let initial_pc = drive.pc();
    for _ in 0..5000 {
        drive.step();
    }
    
    assert_ne!(drive.pc(), initial_pc);
}

#[test]
fn test_drive_boot_behavior() {
    init_logger();
    let rom_array = get_rom();

    // Set up hardware
    static TRACK: ActiveTrack = ActiveTrack::new();
    TRACK.disk_inserted.store(false, Ordering::Relaxed);

    static mut COMMAND_QUEUE: heapless::spsc::Queue<Command, 16> = heapless::spsc::Queue::new();
    let (cmd_tx, mut cmd_rx) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };
    
    let bridge = HardwareBridge::new(&TRACK, cmd_tx);
    let mut drive = Emulator::new(rom_array, bridge);
    
    drive.reset();

    let mut led_on_count = 0;
    let mut head_steps = 0;

    // Run for a significant amount of time to observe boot sequence.
    // The 1541-II flashes the LED several times during self-test.
    for i in 0..1_000_000 {
        drive.step();

        while let Some(cmd) = cmd_rx.dequeue() {
            match cmd {
                Command::SetLed(enabled) => {
                    if enabled { 
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}", i);
                    } else {
                        log::info!("[Test] LED OFF at step {}", i);
                    }
                }
                Command::SetMotor(enabled) => {
                    if enabled { 
                        log::info!("[Test] Motor ON at step {}", i);
                    } else {
                        log::info!("[Test] Motor OFF at step {}", i);
                    }
                }
                Command::StepHead(half_track) => {
                    head_steps += 1;
                    log::info!("[Test] Head Step to {} at step {}", half_track, i);
                }
                _ => {}
            }
        }


        // 1541 boot sequence summary:
        // 1. Initial self-test: Flashes LED.
        // 2. Head-bump: Steps head "out" (lower track IDs) until it hits the stop.
        if led_on_count >= 2 && head_steps > 5 {
            log::info!("Observed expected boot behavior at instruction {}", i);
            return;
        }
    }

    assert!(led_on_count > 0, "Drive LED was never turned on (flashed) during boot");
    // Motor and head steps might take longer than 1M instructions depending on ROM revision
    // but the LED flash happens very early.
}
