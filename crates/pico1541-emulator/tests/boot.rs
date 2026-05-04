mod common;

use pico1541_common::iec_com::{IecCommander, IecError};
use pico1541_common::interface::DriveEvent;
use pollster::block_on;
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

    let boot_cycles = bridge.cycles();
    println!("Drive boot test took {} cycles", boot_cycles);
    assert!(boot_cycles > 10_000); // Just check we actually ticked
}

#[test]
fn test_drive_boot_behavior() {
    // Instruction tracing for debugging
    fn boot_debug_hook(cpu: &racc6502::cpu::CPU, _data: *mut core::ffi::c_void) {
        match cpu.pc {
            0xEAA0 | 0xEB22 | 0xF259 | 0xF263 => {
                log::info!(
                    "PC:${:04X} IR:${:02X} A:${:02X} X:${:02X} Y:${:02X} S:${:02X} P:${:02X}",
                    cpu.pc,
                    cpu.ir,
                    cpu.a,
                    cpu.x,
                    cpu.y,
                    cpu.s,
                    cpu.p
                );
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
                        log::info!(
                            "[Test] LED ON at step {}, cycle {}, count={}",
                            i,
                            total_cycles,
                            led_on_count
                        );
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
                    log::info!(
                        "[Test] Motor changed to {} at step {}, cycle {}",
                        if enabled { "ON" } else { "OFF" },
                        i,
                        total_cycles
                    );
                }
                DriveEvent::HeadStepped { new_half_track } => {
                    stepper_count += 1;
                    log::info!(
                        "[Test] HeadStepped to half-track {} at step {}, cycle {}",
                        new_half_track,
                        i,
                        total_cycles
                    );
                }
                _ => {}
            }
        }

        // If we reached the idle loop, we can stop early
        if drive.pc() >= common::IDLE_LOW && drive.pc() <= common::IDLE_HIGH {
            log::info!(
                "[Test] Reached idle loop at step {}, cycle {}",
                i,
                total_cycles
            );
            break;
        }
    }

    assert_eq!(
        motor_on_count, 1,
        "Expected exactly one Motor ON event during boot (got {})",
        motor_on_count
    );
    assert_eq!(
        motor_off_count, 1,
        "Expected exactly one Motor OFF event during boot (got {})",
        motor_off_count
    );
    assert_eq!(
        stepper_count, 2,
        "Expected exactly two Stepper events during boot (got {})",
        stepper_count
    );
    assert_eq!(
        led_on_count, 0,
        "Expected zero LED ON events during boot (got {})",
        led_on_count
    );
}

#[test]
fn test_dir_no_disk() {
    fn debug_hook(cpu: &racc6502::cpu::CPU, _data: *mut core::ffi::c_void) {
        if cpu.pc >= 0x8000 {
            // Log periodically or on interesting addresses
            if cpu.pc == 0xEA00 || cpu.pc == 0xEBFF || cpu.pc == 0xF2B0 {
                 log::info!("PC:${:04X} A:${:02X} X:${:02X} Y:${:02X} P:${:02X}", cpu.pc, cpu.a, cpu.x, cpu.y, cpu.p);
            }
        }
    }

    let (mut drive, mut bridge) = common::setup_drive(true, 8, Some(debug_hook));

    // Ensure NO DISK is present
    bridge.track.disk_inserted = false;
    bridge.events.clear();

    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        log::info!("Sending '$' command...");
        IecCommander::new(&mut bus)
            .open(8, 0xF0)
            .send(b"$")
            .execute()
            .await?;

        log::info!("Command sent, waiting for status...");

        let mut status_vec = Vec::new();
        IecCommander::new(&mut bus)
            .open(8, 15)
            .receive(|b| status_vec.push(b))
            .await?;
        let status = String::from_utf8_lossy(&status_vec)
            .trim_end_matches('\r')
            .to_string();
        log::info!("Status received: {}", status);

        assert!(
            !status.starts_with("00,"),
            "Expected an error status, got: {:?}",
            status
        );
        Ok::<(), IecError>(())
    })
    .unwrap();
}

#[test]
fn test_dir_listing_virtual() {
    use pico1541_common::formats::{DiskImage, VirtualDirImage};
    use pico1541_common::storage::{DirectoryEntry, DirectorySource};

    struct TestSource {
        name: [u8; 16],
        entries: Vec<DirectoryEntry>,
        pos: usize,
    }

    impl DirectorySource for TestSource {
        fn name(&self) -> &[u8; 16] {
            &self.name
        }
        fn next_entry(&mut self) -> Option<DirectoryEntry> {
            if self.pos < self.entries.len() {
                let res = Some(self.entries[self.pos]);
                self.pos += 1;
                res
            } else {
                None
            }
        }
        fn rewind(&mut self) {
            self.pos = 0;
        }
    }

    let mut entries = Vec::new();
    for i in 1..=8 {
        let name = format!("FILE{:02}", i);
        entries.push(DirectoryEntry::new(name.as_bytes(), 17, i as u8, i as u16));
    }

    let mut name = [0xA0; 16];
    name[..12].copy_from_slice(b"VIRTUAL DISK");
    let source = TestSource {
        name,
        entries,
        pos: 0,
    };
    let mut vdir = VirtualDirImage::new(source);
    vdir.set_id([0x31, 0x31]);

    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    let mut gcr_data = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
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

    log::info!(
        "Starting directory LOAD... Current half-track: {}",
        drive.half_track()
    );

    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 0xF0)
            .send(b"$")
            .execute()
            .await?;

        bus.bridge.ticks(bus.drive, 1_000_000);

        let mut received = Vec::new();
        IecCommander::new(&mut bus)
            .open(8, 0)
            .receive(|b| received.push(b))
            .await?;

        log::info!("Received {} bytes of directory data", received.len());
        let text = String::from_utf8_lossy(&received);
        assert!(text.contains("FILE01"), "Directory listing missing FILE01");
        assert!(
            text.contains("VIRTUAL DISK"),
            "Directory listing missing disk name"
        );
        Ok::<(), IecError>(())
    })
    .unwrap();
}

#[test]
fn test_dir_listing_g64() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8, None);

    bridge.mount_g64("tests/data/test.g64");

    log::info!(
        "Starting G64 directory LOAD... Current half-track: {}",
        drive.half_track()
    );

    block_on(async {
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0xF0)
                .send(b"$")
                .execute()
                .await?;
        }

        let mut status_vec = Vec::new();
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 15)
                .receive(|b| status_vec.push(b))
                .await?;
        }
        let status = String::from_utf8_lossy(&status_vec)
            .trim_end_matches('\r')
            .to_string();
        log::info!("Drive Status: {:?}", status);

        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        let mut received = Vec::new();
        IecCommander::new(&mut bus)
            .open(8, 0)
            .receive(|b| received.push(b))
            .await?;

        log::info!("Received {} bytes of G64 directory data", received.len());
        let text = String::from_utf8_lossy(&received);
        assert!(text.contains("GAME"), "Directory listing missing GAME");
        Ok::<(), IecError>(())
    })
    .unwrap();
}
