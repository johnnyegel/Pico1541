mod common;

use common::TestBridge;
use pico1541_emulator::Emulator;
use pico1541_emulator::interface::DriveEvent;
use racc6502::cpu::CycleState;

fn init_logger() {
    // We must ensure the logger is initialized only once across all tests.
    let _ = env_logger::builder()
        .filter_module("mos6502", log::LevelFilter::Warn)
        .filter_module("pico1541_emulator::systembus", log::LevelFilter::Info)
        .filter_module("pico1541_emulator::drivemechanics", log::LevelFilter::Info)
        .filter_module("pico1541_emulator", log::LevelFilter::Info)
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

    let bridge = TestBridge::new();
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

    let mut bridge = TestBridge::new();
    bridge.track.disk_inserted = false;

    let mut drive = Emulator::new(rom_array, bridge);

    // Instruction tracing for debugging
    fn boot_debug_hook<B: racc6502::bus::Bus>(cpu: &racc6502::cpu::CPU<B>, _data: *mut core::ffi::c_void) {
        match cpu.pc {
            0xEAA0 | 0xEB22 | 0xF259 | 0xF263 => {
                log::info!("PC:${:04X} IR:${:02X} A:${:02X} X:${:02X} Y:${:02X} S:${:02X} P:${:02X}", cpu.pc, cpu.ir, cpu.a, cpu.x, cpu.y, cpu.s, cpu.p);
            }
            _ => {}
        }
    }
    drive.cpu.debug_hook = Some(boot_debug_hook);

    drive.reset();

    let mut led_on_count = 0;
    let mut led_off_count = 0;
    let mut motor_on_count = 0;
    let mut motor_off_count = 0;

    let mut min_half_track = drive.half_track();
    let mut total_cycles: u64 = 0;

    // Run for 3 million instructions (~3s of real drive time)
    for i in 0..3_000_000 {
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

        let events = std::mem::take(&mut drive.cpu.bus.hardware.events);
        for event in events {
            match event {
                DriveEvent::LedChanged(enabled) => {
                    if enabled {
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}, cycle {}, count={}", i, total_cycles, led_on_count);
                    } else {
                        led_off_count += 1;
                        log::info!("[Test] LED OFF at step {}, cycle {}", i, total_cycles);
                    }
                }
                DriveEvent::MotorChanged(enabled) => {
                    if enabled {
                        motor_on_count += 1;
                    } else {
                        motor_off_count += 1;
                    }
                    log::info!("[Test] Motor changed to {} at step {}, cycle {}", if enabled { "ON" } else { "OFF" }, i, total_cycles);
                }
                DriveEvent::HeadStepped { new_half_track } => {
                    log::info!("[Test] HeadStepped to half-track {} at step {}, cycle {}", new_half_track, i, total_cycles);
                }
            }
        }
    }

    assert_eq!(motor_on_count, 1, "Expected exactly one Motor ON event during boot");
    assert_eq!(motor_off_count, 1, "Expected exactly one Motor OFF event during boot");
    assert_eq!(led_on_count, 0, "Expected exactly zero LED ON event during boot");
    assert_eq!(led_off_count, 0, "Expected exactly zero LED OFF event during boot");
}
