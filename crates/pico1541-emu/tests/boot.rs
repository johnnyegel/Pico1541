use pico1541_emu::Emulator;
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::HardwareBridge;
use pico1541_emu::Command;
use core::sync::atomic::Ordering;

fn init_logger() {
    // We must ensure the logger is initialized only once across all tests.
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
    let mut min_half_track = drive.half_track();
    let mut last_logged_ht = 255;

    // Run for 10 million instructions (~10s of real drive time)
    for i in 0..10_000_000 {
        drive.step();

        let ht = drive.half_track();
        if ht < min_half_track {
            min_half_track = ht;
        }
        
        if ht != last_logged_ht {
            log::info!("[Test] Half-Track changed to {} at step {}", ht, i);
            last_logged_ht = ht;
        }

        while let Some(cmd) = cmd_rx.dequeue() {
            match cmd {
                Command::SetLed(enabled) => {
                    if enabled { 
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}, count={}", i, led_on_count);
                    } else {
                        log::info!("[Test] LED OFF at step {}", i);
                    }
                }
                Command::SetMotor(enabled) => {
                    log::info!("[Test] Motor changed to {} at step {}", if enabled { "ON" } else { "OFF" }, i);
                }
                _ => {}
            }
        }

        // Periodically log progress every 1 million steps
        if i % 1_000_000 == 0 {
            log::info!("Progress: step {}, PC=${:04X}, HT={}", i, drive.pc(), ht);
        }

        // If we reached half-track 0, we've successfully homed!
        if ht == 0 {
            log::info!("SUCCESS: Reached Track 0 at step {}", i);
            return;
        }
    }

    log::error!("FAILURE: Never reached track 0. Final PC=${:04X}, Min HT={}, LED Count={}", 
               drive.pc(), min_half_track, led_on_count);
    
    panic!("Boot sequence failed to home head");
}
