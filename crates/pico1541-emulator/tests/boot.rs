mod common;

use pico1541_emulator::interface::DriveEvent;
use racc6502::cpu::CycleState;


#[test]
fn test_drive_boot() {
    let (mut drive, mut bridge) = common::setup_drive(false, 8, None);
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

    let (mut drive, mut bridge) = common::setup_drive(true, 8, Some(boot_debug_hook));

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
                DriveEvent::IecCommand { .. } => {}
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
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    // Ensure NO DISK is present
    bridge.track.disk_inserted = false;
    bridge.events.clear();

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x20 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0xF0, false); 
    bridge.atn_end(&mut drive);
    bridge.send_byte(&mut drive, b'$', true);  

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); 
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);
    
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x40 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0x60, false); 
    bridge.atn_end(&mut drive);

    let _ = bridge.try_turnaround(&mut drive);
    bridge.iec_end(&mut drive);

    bridge.untalk(&mut drive);
    bridge.close_channel(&mut drive, 0);

    let status = bridge.read_status(&mut drive);
    assert!(!status.starts_with("00,"), "Expected an error status, got: {:?}", status);
}

#[test]
fn test_dir_listing_virtual() {
    use pico1541_common::storage::{DirectoryEntry, DirectorySource};
    use pico1541_common::formats::{DiskImage, VirtualDirImage};
    use futures::executor::block_on;
    use std::rc::Rc;
    use std::cell::RefCell;

    struct TestSource {
        name: [u8; 16],
        entries: Vec<DirectoryEntry>,
        pos: usize,
    }

    impl DirectorySource for TestSource {
        fn name(&self) -> &[u8; 16] { &self.name }
        fn next_entry(&mut self) -> Option<DirectoryEntry> {
            if self.pos < self.entries.len() {
                let res = Some(self.entries[self.pos]);
                self.pos += 1;
                res
            } else { None }
        }
        fn rewind(&mut self) { self.pos = 0; }
    }

    let mut entries = Vec::new();
    for i in 1..=8 {
        let name = format!("FILE{:02}", i);
        entries.push(DirectoryEntry::new(name.as_bytes(), 17, i as u8, i as u16));
    }

    let mut name = [0xA0; 16];
    name[..12].copy_from_slice(b"VIRTUAL DISK");
    let source = TestSource { name, entries, pos: 0 };
    let mut vdir = VirtualDirImage::new(source);
    vdir.set_id([0x31, 0x31]); 

    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    let mut gcr_data = [0u8; pico1541_emulator::interface::MAX_TRACK_BYTES];
    // Fill with noise
    gcr_data.fill(0x55);
    
    let layout = block_on(vdir.read_track(34, &mut gcr_data)).expect("read_track failed");
    let bit_len = match layout {
        pico1541_common::formats::TrackLayout::Warp(b) => b,
        pico1541_common::formats::TrackLayout::Accurate(b) => b,
        _ => panic!("Expected data layout"),
    };

    bridge.track.gcr_data.copy_from_slice(&gcr_data);
    bridge.track.bit_length = bit_len;
    bridge.track.disk_inserted = true;
    bridge.track.is_accurate = false; // Use warp mode

    log::info!("Starting directory LOAD... Current half-track: {}", drive.half_track());

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x20 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0xF0, false); 
    bridge.atn_end(&mut drive);
    bridge.send_byte(&mut drive, b'$', true);  
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); 
    bridge.atn_end(&mut drive);

    bridge.ticks(&mut drive, 1_000_000);

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x40 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0x60, false); 
    bridge.atn_end(&mut drive);
    
    bridge.turnaround(&mut drive);

    let received = bridge.receive_all(&mut drive);
    bridge.iec_end(&mut drive);

    log::info!("Received {} bytes of directory data", received.len());
    let text = String::from_utf8_lossy(&received);
    assert!(text.contains("FILE01"), "Directory listing missing FILE01");
    assert!(text.contains("VIRTUAL DISK"), "Directory listing missing disk name");
}

#[test]
fn test_dir_listing_g64() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    bridge.mount_g64("tests/data/test2.g64");

    log::info!("Starting G64 directory LOAD... Current half-track: {}", drive.half_track());

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x20 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0xF0, false); 
    bridge.atn_end(&mut drive);
    bridge.send_byte(&mut drive, b'$', true);  
    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x3F, false); 
    bridge.atn_end(&mut drive);
    bridge.iec_end(&mut drive);

    bridge.ticks(&mut drive, 5_000_000);

    let status = bridge.read_status(&mut drive);
    log::info!("Drive Status: {:?}", status);

    bridge.atn_start(&mut drive);
    bridge.send_byte(&mut drive, 0x40 | bridge.device_id, false); 
    bridge.send_byte(&mut drive, 0x60, false); 
    bridge.atn_end(&mut drive);
    
    bridge.turnaround(&mut drive);

    let received = bridge.receive_all(&mut drive);
    bridge.iec_end(&mut drive);

    log::info!("Received {} bytes of G64 directory data", received.len());
    let text = String::from_utf8_lossy(&received);
    assert!(text.contains("GAME"), "Directory listing missing GAME");
}
