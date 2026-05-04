use pico1541_common::formats::{DiskImage, D64Image, G64Image, TrackLayout};
use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::storage::MemoryStorage;
use pico1541_emulator::drivemechanics::DriveMechanics;
use pico1541_emulator::via6522::Via6522;
use pollster::block_on;
use std::fs;

struct MockHardware {
    track: ActiveTrack,
}

impl MockHardware {
    fn new() -> Self {
        Self {
            track: ActiveTrack::new(),
        }
    }
}

impl IecBus for MockHardware {
    fn iec_host(&self) -> IecBusState {
        IecBusState::default()
    }
    fn iec_drive(&mut self, _state: IecBusState) {}
}

impl HostInterface for MockHardware {
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        &mut self.track
    }
    fn send_event(&mut self, _event: DriveEvent) {}
    fn is_hijacked(&self) -> bool {
        false
    }
    fn pop_discard_request(&mut self) -> bool {
        false
    }
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}
    fn sync(&mut self) {}
}

#[test]
fn test_g64_track_reading_low_level() {
    let mut hw = MockHardware::new();
    let mut via2 = Via6522::new();
    let mut mechanics = DriveMechanics::new();

    // 1. Load G64 Track 0 (HT 0)
    let g64_data = fs::read("tests/data/test.g64").expect("Failed to read G64");
    let mut storage = MemoryStorage::new(&g64_data);
    let mut g64 = G64Image::new(&mut storage);

    let mut track_data = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
    let layout = block_on(g64.read_track(0, &mut track_data)).expect("Failed to read track 0");

    if let TrackLayout::Accurate(bits) = layout {
        hw.track.gcr_data.copy_from_slice(&track_data);
        hw.track.bit_length = bits;
        hw.track.is_formatted = true;
        hw.track.is_accurate = true;
        hw.track.disk_inserted = true;
    } else {
        panic!("Expected accurate track layout for G64 (Layout was {:?})", layout);
    }

    // 2. Setup Drive State
    // Motor ON (PB2), Bitrate Zone 3 (PB5=1, PB6=1) for Track 0
    // PB4 (R/W) = 1 (Read)
    via2.orb = 0b01110100; // Bit 6=1, Bit 5=1, Bit 4=1, Bit 2=1
    via2.ddrb = 0xFF;

    // 3. Spin up motor
    for _ in 0..250_001 {
        mechanics.tick(&mut hw, &mut via2);
    }
    assert!(mechanics.motor_spun_up);

    // 4. Capture one revolution
    let mut captured_bytes = Vec::new();
    let mut sync_assertions = 0;
    let mut in_sync = false;
    let mut byte_ready_pulses = 0;

    // A full revolution at 300 RPM (5Hz) is 200,000 cycles at 1MHz.
    // At Zone 3 (26 cycles/byte), that's ~7692 bytes.
    for i in 0..200_000 {
        let byte_ready = mechanics.tick(&mut hw, &mut via2);

        let sync_active = (via2.in_b & 0x80) != 0;

        if sync_active && !in_sync {
            sync_assertions += 1;
            in_sync = true;
        } else if !sync_active && in_sync {
            in_sync = false;
        }

        if byte_ready {
            byte_ready_pulses += 1;
            captured_bytes.push(via2.in_a);
            
            // Success Criteria: Byte Ready MUST NOT pulse during SYNC
            assert!(!sync_active, "Byte Ready pulsed during SYNC at cycle {}", i);
        }
    }

    println!("Captured {} bytes", captured_bytes.len());
    println!("SYNC assertions: {}", sync_assertions);
    println!("Byte Ready pulses: {}", byte_ready_pulses);

    // Track 0 has 21 sectors * 2 blocks = 42 sync marks.
    assert!(sync_assertions >= 42, "Should have at least 42 sync marks for 21 sectors (got {})", sync_assertions);

    // Verify framing
    let mut headers = 0;
    let mut data_blocks = 0;
    
    for &b in &captured_bytes {
        if b == 0x52 { headers += 1; }
        if b == 0x4A { data_blocks += 1; }
    }

    assert!(headers >= 21, "Should find at least 21 headers (got {})", headers);
    assert!(data_blocks >= 21, "Should find at least 21 data blocks (got {})", data_blocks);
}

#[test]
fn test_compare_d64_g64_track_18() {
    // 1. Load G64 Track 18
    let g64_data = fs::read("tests/data/test.g64").expect("Failed to read G64");
    let mut storage_g64 = MemoryStorage::new(&g64_data);
    let mut g64 = G64Image::new(&mut storage_g64);

    let mut g64_track_data = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
    // In this specific test.g64, Track 18 is at HT 68.
    let g64_layout = block_on(g64.read_track(68, &mut g64_track_data)).expect("Failed to read G64 track 18");

    // 2. Load D64 Track 18 (HT 34)
    let d64_data = fs::read("tests/data/test.d64").expect("Failed to read D64");
    let mut storage_d64 = MemoryStorage::new(&d64_data);
    let mut d64 = D64Image::new(&mut storage_d64);
    
    let mut d64_track_data = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
    let d64_layout = block_on(d64.read_track(34, &mut d64_track_data)).expect("Failed to read D64 track 18");

    println!("G64 Track 18 layout: {:?}", g64_layout);
    println!("D64 Track 18 layout: {:?}", d64_layout);

    // 3. Analysis
    let g64_headers = count_headers(&g64_track_data);
    let d64_headers = count_headers(&d64_track_data);

    println!("G64 Headers: {}", g64_headers);
    println!("D64 Headers: {}", d64_headers);

    assert_eq!(d64_headers, 19, "D64 Track 18 should have 19 sectors");
    assert!(g64_headers >= 19, "G64 Track 18 should have at least 19 sectors");
}

fn count_headers(data: &[u8]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < data.len() - 5 {
        if data[i] == 0xFF && data[i+1] == 0xFF {
            while i < data.len() && data[i] == 0xFF {
                i += 1;
            }
            if i < data.len() {
                if data[i] == 0x52 {
                    count += 1;
                }
            }
        } else {
            i += 1;
        }
    }
    count
}
