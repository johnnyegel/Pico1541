mod common;

use pico1541_emulator::interface::DriveEvent;
use racc6502::cpu::CycleState;


#[test]
fn test_drive_boot() {
    let (mut drive, mut bridge) = common::setup_drive(false, None);
    drive.reset();

    let initial_pc = drive.pc();
    for _ in 0..5000 {
        drive.step(&mut bridge);
    }

    assert_ne!(drive.pc(), initial_pc);
}

#[test]
fn test_drive_boot_behavior() {
    // Instruction tracing for debugging
    fn boot_debug_hook(cpu: &racc6502::cpu::CPU, _data: *mut core::ffi::c_void) {
        match cpu.pc {
            0xEAA0 | 0xEB22 | 0xF259 | 0xF263 => {
                log::info!("PC:${:04X} IR:${:02X} A:${:02X} X:${:02X} Y:${:02X} S:${:02X} P:${:02X}", cpu.pc, cpu.ir, cpu.a, cpu.x, cpu.y, cpu.s, cpu.p);
            }
            _ => {}
        }
    }

    let (mut drive, mut bridge) = common::setup_drive(true, Some(boot_debug_hook));

    let mut led_on_count = 0;
    let mut motor_on_count = 0;
    let mut motor_off_count = 0;
    let mut stepper_count = 0;

    let mut min_half_track = drive.half_track();
    let mut total_cycles: u64 = 0;

    // Run for 3 million instructions (~3s of real drive time)
    for i in 0..3_000_000 {
        // Run until we start fetching the next opcode (Sync point)
        loop {
            drive.tick(&mut bridge);
            total_cycles += 1;
            if drive.cpu.state == CycleState::FetchOpcode && drive.cpu.step_index == 0 {
                break;
            }
        }

        let ht = drive.half_track();
        if ht < min_half_track {
            min_half_track = ht;
        }

        let events = std::mem::take(&mut bridge.events);
        for event in events {
            match event {
                DriveEvent::LedChanged(enabled) => {
                    if enabled {
                        led_on_count += 1;
                        log::info!("[Test] LED ON at step {}, cycle {}, count={}", i, total_cycles, led_on_count);
                    } else {
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
                    stepper_count += 1;
                    log::info!("[Test] HeadStepped to half-track {} at step {}, cycle {}", new_half_track, i, total_cycles);
                }
            }
        }
        
        // If we reached the idle loop, we can stop early
        if drive.pc() >= common::IDLE_LOW && drive.pc() <= common::IDLE_HIGH {
            log::info!("[Test] Reached idle loop at step {}, cycle {}", i, total_cycles);
            break;
        }
    }

    assert_eq!(motor_on_count, 1, "Expected exactly one Motor ON event during boot (got {})", motor_on_count);
    assert_eq!(motor_off_count, 1, "Expected exactly one Motor OFF event during boot (got {})", motor_off_count);
    assert_eq!(stepper_count, 2, "Expected exactly two Stepper events during boot (got {})", stepper_count);
    assert_eq!(led_on_count, 0, "Expected zero LED ON events during boot (got {})", led_on_count);
}

#[test]
fn test_dir_no_disk() {
    let (mut drive, mut bridge) = common::setup_drive(true, None);

    // Ensure NO DISK is present
    bridge.track.disk_inserted = false;
    bridge.events.clear();

    // Send the DIRECTORY ("$") command on channel 0.
    // With no disk the ROM will spin the motor, try to find track 18, 
    // fail, and eventually rattle the head against the stop (head banding).
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x28, false); // LISTEN 8
    bridge.send_byte(&mut drive, 0x60, true);  // SEC 0 (Load/Open "$")
    bridge.atn_stop(&mut drive);

    bridge.send_byte(&mut drive, b'$', true);  // "$"

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, true); // UNLISTEN
    
    log::info!("Sent DIR command with no disk. Waiting for drive to process...");

    // Read the status response from channel 15.
    let status = {
        bridge.send_byte(&mut drive, 0x48, false); // TALK 8
        bridge.send_byte(&mut drive, 0x6F, true);  // SEC 15
        
        bridge.atn_stop(&mut drive);
        bridge.turnaround(&mut drive);
        
        let bytes = bridge.receive_all(&mut drive);
        bridge.end_communication(&mut drive);
        String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_string()
    };

    let final_half_track = drive.half_track();
    log::info!("Half-track after failed DIR: {}", final_half_track);
    log::info!("Status: {}", status);

    // Drive should have tried to read and failed — motor events and head movement
    let motor_events: usize = bridge.events.iter()
        .filter(|e| matches!(e, pico1541_emulator::interface::DriveEvent::MotorChanged(_)))
        .count();
    let step_events: usize = bridge.events.iter()
        .filter(|e| matches!(e, pico1541_emulator::interface::DriveEvent::HeadStepped { .. }))
        .count();
    log::info!("Motor events: {}, Step events: {}", motor_events, step_events);

    assert!(motor_events >= 1, "Expected motor ON event, got {}", motor_events);
    // A rattle (banding) involves many steps towards track 0.
    assert!(step_events > 5, "Expected significant head movement (rattle), got only {} steps", step_events);
    assert!(!status.starts_with("00,"), "Expected an error status, got: {:?}", status);
}
