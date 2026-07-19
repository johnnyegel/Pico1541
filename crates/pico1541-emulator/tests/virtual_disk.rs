mod common;
use pico1541_common::interface::DriveEvent;
use common::*;
use pico1541_common::formats::{AnyDiskImage, VirtualDiskImage};
use pico1541_common::storage::DummySource;

#[test]
fn test_virtual_disk_skeleton_orientation() {
    let (mut drive, mut bridge) = setup_drive(true, 8);

    // Mount VirtualDiskImage
    let mut disk = VirtualDiskImage::<pico1541_common::storage::MemoryStorage<'static>, DummySource>::new(DummySource, b"TEST");
    disk.set_id([0x56, 0x44]);
    let mut runner = DiskRunner {
        image: AnyDiskImage::VirtualDisk(disk),
        last_half_track: 255,
    };
    
    bridge.sync_hook = Some(Box::new(move |track| {
        runner.sync(track);
    }));

    // Start on Track 17 (HT 34)
    assert_eq!(bridge.buffer.split().0.get_active_half_track(), 34);
    
    // Patch ID in RAM to match the VirtualDiskImage default ID [0x56, 0x44] ("VD")
    // $12-$13 and $16-$17 are used for ID verification in 1541 ROM.
    drive.bus_mut().memory[0x0012] = 0x56;
    drive.bus_mut().memory[0x0013] = 0x44;
    drive.bus_mut().memory[0x0016] = 0x56;
    drive.bus_mut().memory[0x0017] = 0x44;

    // We want to verify that the drive can orient itself on a non-directory track.
    // We'll use a "Job" to read a sector on Track 10.
    // 1541 Job Queue starts at $01. Job 1 is at $01.
    // $00: Job Track
    // $01: Job Sector / Job Command
    
    let track_10 = 10;
    let sector_0 = 0;
    
    drive.bus_mut().memory[0x0008] = track_10; // Track for job 1
    drive.bus_mut().memory[0x0009] = sector_0; // Sector for job 1
    
    log::info!("Current DOS track ($22): {}", drive.bus_mut().memory[0x0022]);
    drive.bus_mut().memory[0x0022] = 18; // FORCE it to 18 so it knows to step!

    drive.bus_mut().memory[0x0001] = 0x80;     // Execute READ job (0x80 = Read)

    // Run for a while. The drive should:
    // 1. See the job
    // 2. Step the head to Track 10
    // 3. Find the skeleton headers
    // 4. Successfully complete the job (Job status will be 1 for success)
    
    let mut success = false;
    for i in 0..2_000_000 {
        drive.tick(&mut bridge);
        
        let status = drive.bus_mut().memory[0x0001];
        if status == 0x01 {
            success = true;
            log::info!("Job completed successfully at tick {}", i);
            break;
        }
        if status > 0x01 && status < 0x80 {
            panic!("Job failed with status ${:02X} at tick {}", status, i);
        }
    }
    
    assert!(success, "Drive failed to orient and read sector 0 on Track 10 (skeleton track)");
}

#[test]
#[ignore = "FIXME: Will fix later"]
fn test_virtual_disk_stridden_warp() {
    let (mut drive, mut bridge) = setup_drive(true, 8);

    let disk = VirtualDiskImage::<pico1541_common::storage::MemoryStorage<'static>, DummySource>::new(DummySource, b"TEST");
    let mut runner = DiskRunner {
        image: AnyDiskImage::VirtualDisk(disk),
        last_half_track: 255,
    };
    
    bridge.sync_hook = Some(Box::new(move |track| {
        runner.sync(track);
    }));

    // Step to Track 1 (HT 0)
    // We'll force the state manually
    drive.bus_mut().memory[0x0022] = 1;
    drive.bus_mut().mechanics.halftrack_head_pos = 2;
    {
        let manager = bridge.buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        inactive.track_id = 2;
        manager.commit_dormant_buffer();
    }
    
    // Call sync_hook manually to load the track data since we forced the head position
    bridge.sync_hook.as_mut().unwrap()(bridge.buffer);
    
    // Turn motor ON (VIA2 PB2)
    drive.bus_mut().via2.orb |= 0x04;
    // Set CA2 HIGH (Enable SO / Reading)
    drive.bus_mut().via2.pcr |= 0x0E;
    
    // Initial tick to load track
    drive.tick(&mut bridge);
    
    // Set requested sector to 5
    drive.bus_mut().memory[0x000C] = 5;
    
    // Tick enough to let the warp happen (it happens on the next byte ready)
    // Since we just started, it will take some cycles to reach the first byte ready.
    for _ in 0..1000 {
        drive.tick(&mut bridge);
    }
    
    let pos = drive.bus_mut().mechanics.track_byte_pos_ix();
    log::info!("Position after sector 5 warp: {}", pos);
    // Sector 5 * Stride 372 = 1860. It may have advanced a few bytes.
    assert!(pos >= 1860 && pos < 2000, "Warp to sector 5 failed, pos={}", pos);
    
    // Change to sector 10
    drive.bus_mut().memory[0x000C] = 10;
    for _ in 0..1000 {
        drive.tick(&mut bridge);
    }
    let pos = drive.bus_mut().mechanics.track_byte_pos_ix();
    log::info!("Position after sector 10 warp: {}", pos);
    assert!(pos >= 3720 && pos < 3900, "Warp to sector 10 failed, pos={}", pos);
}

#[test]
fn test_virtual_disk_load_interception() {
    let (mut drive, mut bridge) = setup_drive(false, 8);

    // Perform manual reset
    reset_drive(&mut drive, &mut bridge);

    /*
    // Mount VirtualDiskImage AFTER reset (since reset clears sync_hook)
    let mut disk = VirtualDiskImage::new(DummySource);
    // ...
    */

    // Simulate C64 sending OPEN 8,15
    pollster::block_on(async {
        let mut bus = IecTestBridge {
            drive: &mut drive,
            bridge: &mut bridge,
        };
        
        pico1541_common::iec_com::IecCommander::new(&mut bus)
            .open(8, 15)
            .send(b"GAME")
            .execute()
            .await
            .unwrap();
    });

    // Check if the bridge captured the IecCommand event
    let command_event = bridge.events.iter().find(|e| matches!(e, DriveEvent::IecCommand { .. }));
    
    assert!(command_event.is_some(), "No IecCommand event was emitted");
    if let Some(DriveEvent::IecCommand { data, len }) = command_event {
        let cmd = std::str::from_utf8(&data[..*len as usize]).unwrap();
        assert_eq!(cmd, "GAME");
        log::info!("Intercepted command: {}", cmd);
    }
}

#[test]
fn test_virtual_disk_shortcut_generation() {
    use pico1541_common::storage::DirectoryEntry;
    use pico1541_common::DiskImage;

    let source = DummySource;
    let mut vdir = VirtualDiskImage::<pico1541_common::storage::MemoryStorage<'static>, DummySource>::new(source, b"TEST");
    
    // Set a shortcut entry
    let entry = DirectoryEntry::new(b"MYGAME", 1, 0, 100);
    vdir.set_shortcut_entry(Some(entry));
    assert_eq!(vdir.shortcut_entry.unwrap().filename[0..6], *b"MYGAME");
    
    let mut dest = vec![0u8; 10000];
    let result = pollster::block_on(vdir.read_track(36, &mut dest)); // HT 36 = Track 18
    assert!(result.is_ok());
}
