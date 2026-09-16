use pico1541_common::iec_com::{IecCommander, IecError};
use pico1541_common::track_buffer::TrackLayout;
use pico1541_emulator::MemoryMonitor;
use pollster::block_on;

mod common;

#[test]
fn test_drive_boot() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    let mut cycles = 0;
    while cycles < 25_000_000 {
        let pc = drive.pc();
        if pc >= common::IDLE_LOW && pc <= common::IDLE_HIGH {
            break;
        }
        drive.tick(&mut bridge);
        cycles += 1;
    }

    log::info!("Drive boot test took {} cycles", cycles);
    let pc = drive.pc();
    assert!(
        pc >= common::IDLE_LOW && pc <= common::IDLE_HIGH,
        "Drive did not reach idle loop within timeout (PC=${:04X})",
        pc
    );
}

#[test]
fn test_dir_no_disk() {
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    block_on(async {
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };

            // Try to read directory from empty drive
            IecCommander::new(&mut bus)
                .open(8, 0xF0)
                .send(b"$")
                .execute()
                .await?;
        }

        bridge.ticks(&mut drive, 25_000_000);

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
#[ignore = "FIXME: Will fix later"]
fn test_dir_listing_virtual() {
    use pico1541_manager::formats::{DiskImage, VirtualDiskImage};
    use pico1541_manager::storage::{DirectoryEntry, DirectorySource};

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
                let entry = self.entries[self.pos].clone();
                self.pos += 1;
                Some(entry)
            } else {
                None
            }
        }
        fn rewind(&mut self) {
            self.pos = 0;
        }
    }

    let mut name = [0u8; 16];
    name[0..12].copy_from_slice(b"VIRTUAL DISK");
    let mut entries = Vec::new();
    entries.push(DirectoryEntry {
        filename: *b"GAME            ",
        block_size: 10,
        file_type: 0x82, // PRG
        first_track: 18,
        first_sector: 1,
    });
    entries.push(DirectoryEntry {
        filename: *b"FILE01          ",
        block_size: 20,
        file_type: 0x82, // PRG
        first_track: 18,
        first_sector: 3,
    });

    let source = TestSource {
        name,
        entries,
        pos: 0,
    };
    let mut vdir =
        VirtualDiskImage::<pico1541_manager::storage::MemoryStorage<'static>, TestSource>::new(
            source, b"TEST",
        );
    vdir.set_id([0x31, 0x31]);

    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    log::info!("Waiting for drive to reach idle loop...");
    let mut timeout = 0;
    while !bridge.is_idle(&drive) {
        bridge.ticks(&mut drive, 1000);
        timeout += 1;
        if timeout > 5000 {
            panic!(
                "Drive never reached idle loop after reset (PC=${:04X})",
                drive.pc()
            );
        }
    }

    let mut gcr_data = [0u8; 8192];
    // Fill with noise
    gcr_data.fill(0x55);

    let layout = block_on(vdir.read_track(34, &mut gcr_data)).expect("read_track failed");

    println!("MOCK: Loaded track length {}", layout.1);
    {
        let manager = bridge.buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        unsafe { (*inactive.data).copy_from_slice(&gcr_data); }
        inactive.layout = layout.0;
        inactive.length = layout.1;
        inactive.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
            34,
            pico1541_common::track_buffer::default_speed_zone(34),
        );
        manager.commit_dormant_buffer();
    }

    // Use a sync hook to keep the virtual track in sync with head position
    let mut last_ht = 0xFF;
    bridge.sync_hook = Some(Box::new(move |track| {
        let manager = track.split().0;
        let current_ht = manager.get_active_track_identifier().half_track();
        if current_ht != last_ht {
            manager.duplicate_active_to_inactive();
            let b = manager.get_dormant_buffer_mut();
            println!("MOCK HANDLER HT: {}", current_ht);
            if current_ht != 34 {
                b.layout = TrackLayout::Void;
                b.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
                    current_ht,
                    pico1541_common::track_buffer::default_speed_zone(current_ht),
                );
            } else {
                b.layout = layout.0;
                b.length = layout.1;
                b.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
                    current_ht,
                    pico1541_common::track_buffer::default_speed_zone(current_ht),
                );
            }
            manager.commit_dormant_buffer();
            last_ht = current_ht;
        }
    }));

    log::info!(
        "Starting directory LOAD... Current half-track: {}",
        drive.half_track()
    );

    block_on(async {
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0xF0) // OPEN 0 (Directory)
                .send(b"$")
                .execute()
                .await?;
        }

        bridge.ticks(&mut drive, 25_000_000);

        let mut received = Vec::new();
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0) // TALK 0
                .receive(|b| received.push(b))
                .await?;
        }

        let text = String::from_utf8_lossy(&received);
        println!("RECEIVED TEXT: {:?}", text);
        log::info!(
            "Received {} bytes of directory data:\n{}",
            received.len(),
            text
        );
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
    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    bridge.mount_g64("tests/data/test.g64");

    /*
    // Move physical head to Track 18.0 (HT 34)
    drive.bus_mut().mechanics.halftrack_head_pos = 34;
    bridge.track.half_track = 34;
    */

    log::info!("Waiting for drive to reach idle loop...");
    let mut timeout = 0;
    while !bridge.is_idle(&drive) {
        bridge.ticks(&mut drive, 1000);
        timeout += 1;
        if timeout > 5000 {
            panic!(
                "Drive never reached idle loop after reset (PC=${:04X})",
                drive.pc()
            );
        }
    }

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
                .open(8, 0xF0) // OPEN 0
                .send(b"$")
                .execute()
                .await?;
        }

        bridge.ticks(&mut drive, 25_000_000);

        let cmd_len = 255;
        let cmd = &drive.bus().memory[0x0200..0x0200 + cmd_len as usize];
        let erword = drive.bus().memory[0x026C];
        println!("COMMAND LEN: {}", cmd_len);
        println!("COMMAND BUF: {:?}", String::from_utf8_lossy(cmd));
        println!("ERROR CODE: {}", erword);

        let mut received = Vec::new();
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0) // TALK 0
                .receive(|b| received.push(b))
                .await?;
        }

        let text = String::from_utf8_lossy(&received);
        println!("RECEIVED TEXT: {:?}", text);
        log::info!(
            "Received {} bytes of G64 directory data.\n{}",
            received.len(),
            text
        );
        assert!(text.contains("GAME"), "Directory listing missing GAME");
        Ok::<(), IecError>(())
    })
    .unwrap();
}

#[test]
#[ignore = "FIXME: Will fix later"]
fn test_dir_listing_stable() {
    use pico1541_common::track_buffer::TrackLayout;
    use pico1541_manager::formats::{DiskImage, VirtualDiskImage};
    use pico1541_manager::storage::{DirectoryEntry, DirectorySource};
    use pollster::block_on;

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
                let e = self.entries[self.pos];
                self.pos += 1;
                Some(e)
            } else {
                None
            }
        }
        fn rewind(&mut self) {
            self.pos = 0;
        }
    }

    let mut name = [0u8; 16];
    name[0..12].copy_from_slice(b"STABLE DISK ");
    let mut entries = Vec::new();
    entries.push(DirectoryEntry {
        filename: *b"GAME            ",
        block_size: 10,
        file_type: 0x82, // PRG
        first_track: 18,
        first_sector: 1,
    });
    entries.push(DirectoryEntry {
        filename: *b"FILE01          ",
        block_size: 20,
        file_type: 0x82, // PRG
        first_track: 18,
        first_sector: 3,
    });

    let source = TestSource {
        name,
        entries,
        pos: 0,
    };
    let mut vdir =
        VirtualDiskImage::<pico1541_manager::storage::MemoryStorage<'static>, TestSource>::new(
            source, b"TEST",
        );
    vdir.set_id([0x31, 0x31]);

    let (mut drive, mut bridge) = common::setup_drive(true, 8);

    log::info!("Waiting for drive to reach idle loop...");
    let mut timeout = 0;
    while !bridge.is_idle(&drive) {
        bridge.ticks(&mut drive, 1000);
        timeout += 1;
        if timeout > 5000 {
            panic!(
                "Drive never reached idle loop after reset (PC=${:04X})",
                drive.pc()
            );
        }
    }

    let mut gcr_data = [0u8; 8192];
    let layout = block_on(vdir.read_track(34, &mut gcr_data)).expect("read_track failed");

    {
        let manager = bridge.buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        unsafe { (*inactive.data).copy_from_slice(&gcr_data); }
        inactive.layout = layout.0;
        inactive.length = layout.1;
        inactive.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
            34,
            pico1541_common::track_buffer::default_speed_zone(34),
        );
        manager.commit_dormant_buffer();
    }

    // Use a sync hook to keep the virtual track in sync with head position
    use std::cell::RefCell;
    use std::rc::Rc;
    let layout_rc = Rc::new(RefCell::new(layout));
    let layout_clone = layout_rc.clone();

    let mut last_ht = 0xFF;
    bridge.sync_hook = Some(Box::new(move |track| {
        let manager = track.split().0;
        let current_ht = manager.get_active_track_identifier().half_track();
        if current_ht != last_ht {
            manager.duplicate_active_to_inactive();
            let b = manager.get_dormant_buffer_mut();
            println!("MOCK HANDLER HT: {}", current_ht);
            if current_ht != 34 {
                b.layout = TrackLayout::Void;
                b.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
                    current_ht,
                    pico1541_common::track_buffer::default_speed_zone(current_ht),
                );
            } else {
                let lc = *layout_clone.borrow();
                b.layout = lc.0;
                b.length = lc.1;
                b.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
                    current_ht,
                    pico1541_common::track_buffer::default_speed_zone(current_ht),
                );
            }
            manager.commit_dormant_buffer();
            last_ht = current_ht;
        }
    }));

    // First listing
    block_on(async {
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0xF0) // OPEN 0 (Directory)
                .send(b"$")
                .execute()
                .await?;
        }

        bridge.ticks(&mut drive, 25_000_000);

        let mut received = Vec::new();
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0) // TALK 0
                .receive(|b| received.push(b))
                .await?;
        }

        let text = String::from_utf8_lossy(&received);
        println!("RECEIVED TEXT: {:?}", text);
        assert!(
            text.contains("FILE01"),
            "Directory listing 1 missing FILE01"
        );
        assert!(
            text.contains("STABLE DISK"),
            "Directory listing 1 missing disk name"
        );
        Ok::<(), IecError>(())
    })
    .unwrap_or_else(|e| {
        for i in 0..128 {
            let row = (i * 16) as u16;
            let mut s = format!("{:04X}: ", row);
            for j in 0..16 {
                s.push_str(&format!(
                    "{:02X} ",
                    drive.bus_mut().peek(row + j, &mut bridge)
                ));
            }
            log::error!("{}", s);
        }
        panic!("Timeout: {:?}", e);
    });

    // Send CLOSE 0 (0xE0)
    block_on(async {
        let mut bus = common::IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        IecCommander::new(&mut bus)
            .open(8, 0xE0) // CLOSE 0
            .execute()
            .await?;
        Ok::<(), IecError>(())
    })
    .unwrap();

    bridge.ticks(&mut drive, 500_000);

    // Call read_track AGAIN to simulate the second load
    let layout2 = block_on(vdir.read_track(34, &mut gcr_data)).expect("read_track failed");
    {
        let manager = bridge.buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        unsafe { (*inactive.data).copy_from_slice(&gcr_data); }
        inactive.layout = layout2.0;
        inactive.length = layout2.1;
        inactive.identifier = pico1541_common::track_buffer::TrackIdentifier::new(
            34,
            pico1541_common::track_buffer::default_speed_zone(34),
        );
        manager.commit_dormant_buffer();
    }
    *layout_rc.borrow_mut() = layout2;

    // Second listing
    block_on(async {
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0xF0) // OPEN 0 (Directory)
                .send(b"$")
                .execute()
                .await?;
        }

        bridge.ticks(&mut drive, 25_000_000);

        let mut received = Vec::new();
        {
            let mut bus = common::IecTestBridge {
                drive: &mut drive,
                bridge: &mut bridge,
            };
            IecCommander::new(&mut bus)
                .open(8, 0) // TALK 0
                .receive(|b| received.push(b))
                .await?;
        }

        let text = String::from_utf8_lossy(&received);
        println!("RECEIVED TEXT: {:?}", text);
        assert!(
            text.contains("FILE01"),
            "Directory listing 2 missing FILE01"
        );
        assert!(
            text.contains("STABLE DISK"),
            "Directory listing 2 missing disk name"
        );
        Ok::<(), IecError>(())
    })
    .unwrap();
}
