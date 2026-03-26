use pico1541_emu::Emulator;
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::HardwareBridge;
use pico1541_emu::Command;
use racc6502::cpu::CycleState;
use core::sync::atomic::Ordering;

fn init_logger() {
    // We must ensure the logger is initialized only once across all tests.
    let _ = env_logger::builder()
        .filter_module("mos6502", log::LevelFilter::Warn)
        .filter_module("pico1541_emu::systembus", log::LevelFilter::Warn)
        .filter_module("pico1541_emu::drivemechanics", log::LevelFilter::Info)
        .filter_module("pico1541_emu", log::LevelFilter::Info)
        .filter_module("boot", log::LevelFilter::Info)
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
 
    // Instruction tracing for debugging
    fn boot_debug_hook<B: racc6502::bus::Bus>(cpu: &racc6502::cpu::CPU<B>, _data: *mut core::ffi::c_void) {
        // Log specific points of interest or a range of addresses
        match cpu.pc {
            // Initialization and Main Loop areas
            //0xEAA0..=0xEFFF | 0xF97E..=0xF9AF | 0xEB22 => {
            0xEAA0 | 0xEB22 | 0xF259 | 0xF263 => {
                log::info!("PC:${:04X} IR:${:02X} A:${:02X} X:${:02X} Y:${:02X} S:${:02X} P:${:02X}", cpu.pc, cpu.ir, cpu.a, cpu.x, cpu.y, cpu.s, cpu.p);
            }
            _ => {}
        }
    }
    drive.cpu.debug_hook = Some(boot_debug_hook);

    drive.reset();

    let mut led_on_count = 0;
    let mut min_half_track = drive.half_track();
    let mut total_cycles: u64 = 0;

    // Run for 10 million instructions (~10s of real drive time)
    for i in 0..10_000_000 { // 10_000_000 {
        // Run until we start fetching the next opcode (Sync point)
        loop {
            drive.cpu.tick();
            total_cycles += 1;
            if drive.cpu.state == CycleState::FetchOpcode && drive.cpu.step_index == 0 {
                break;
            }
        }
        
        let ht = drive.half_track();
        if ht < min_half_track {
            min_half_track = ht;
        }
        
        while let Some(cmd) = cmd_rx.dequeue() {
            match cmd {
                Command::SetLed(enabled) => {
                    if enabled { 
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}, cycle {}, count={}", i, total_cycles, led_on_count);
                    } else {
                        log::info!("[Test] LED OFF at step {}, cycle {}", i, total_cycles);
                    }
                }
                Command::SetMotor(enabled) => {
                    log::info!("[Test] Motor changed to {} at step {}, cycle {}", if enabled { "ON" } else { "OFF" }, i, total_cycles);
                }
                Command::StepHead(ht) => {
                    log::info!("[Test] StepHead to half-track {} at step {}, cycle {}", ht, i, total_cycles);
                }
                _ => {}
            }
        }

        // Periodically log progress every 1 million steps
        if i % 1_000_000 == 0 {
            log::info!("Progress: step {}, cycle {}, PC=${:04X}, HT={}", i, total_cycles, drive.pc(), ht);
        }
    }

    log::error!("FAILURE: Never reached track 0. Final PC=${:04X}, Min HT={}, LED Count={}", 
               drive.pc(), min_half_track, led_on_count);
    
    panic!("Boot sequence failed to home head");
}
