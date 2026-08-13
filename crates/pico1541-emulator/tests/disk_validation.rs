#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::formats::{D64Image, DiskImage, G64Image};
use pico1541_common::track_buffer::TrackLayout;
use pico1541_common::magnetic_surface::{SoftwareMagneticSurface};
use pico1541_common::track_buffer::DualCoreTrackBuffer;
use pico1541_common::interface::{DriveEvent, HostInterface};
use pico1541_common::storage::MemoryStorage;
use pollster::block_on;
use std::fs;

static DUMMY_ROM: [u8; 16384] = [0; 16384];

fn get_system_bus(surface: SoftwareMagneticSurface, handle: pico1541_common::track_buffer::EmulatorHandle) -> pico1541_emulator::systembus::SystemBus<MockHardware, SoftwareMagneticSurface> {
    let mechanics = pico1541_emulator::drivemechanics::DriveMechanics::new(surface, handle);
    pico1541_emulator::systembus::SystemBus::new(&DUMMY_ROM, Box::leak(Box::new([0u8; 32768])), 8, mechanics)
}

struct MockHardware {
    
}

impl MockHardware {
    fn new() -> Self {
        Self {
            
        }
    }

}


impl ViaHardware for MockHardware {}
impl HostInterface for MockHardware {
    
    fn send_event(&mut self, _event: DriveEvent) {}
    fn is_hijacked(&self) -> bool {
        false
    }
    fn pop_discard_request(&mut self) -> bool {
        false
    }
    fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}
    }

#[test]
fn test_g64_track_reading_low_level() {
    let mut hw = MockHardware::new();
    let hw_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new()));
    let handle = hw_buffer.split().1;
    let surface = SoftwareMagneticSurface::new();
    let mut bus = get_system_bus(surface, handle);
    bus.mechanics.force_state_for_testing(0, 3, 0);
    
    

    // 1. Load G64 Track 0 (HT 0)
    let g64_data = fs::read("tests/data/test.g64").expect("Failed to read G64");
    let storage = MemoryStorage::new(&g64_data);
    let mut g64 = block_on(G64Image::from_storage(storage)).expect("Failed to initialize G64Image");

    let mut track_data = [0u8; 8192];
    let layout = block_on(g64.read_track(0, &mut track_data)).expect("Failed to read track 0");

    log::trace!("Track layout from G64: {:?}", layout);

    {
        let manager = hw_buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        inactive.data.copy_from_slice(&track_data);
        manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(2, pico1541_common::track_buffer::default_speed_zone(2)), layout.0, layout.1);
    }
    
    bus.mechanics.halftrack_head_pos = 2;
    
    bus.mechanics.force_state_for_testing(2, 3, 0);

    match layout.0 {
        TrackLayout::Accurate => log::debug!("Track accurate layout"),
        _ => panic!("Expected accurate track layout for G64 (Layout was {:?})", layout)
    };

    // 2. Setup Drive State
    // Motor ON (PB2), Bitrate Zone 3 (PB5=1, PB6=1) for Track 0
    bus.via2.ddrb = 0xFF;
    bus.via2.orb = 0b01100100; // Bit 6=1, Bit 5=1, Bit 2=1 (Motor ON, Zone 0)
    bus.mechanics.update_state(bus.via2.get_pb_out(), &mut hw, 0);
    
    bus.via2.pcr = 0b1110_1110; // Enable Read mode and SO

    // 4. Capture one revolution
    let mut captured_bytes = Vec::new();
    let mut sync_assertions = 0;
    let mut in_sync = false;
    let mut byte_ready_pulses = 0;

    // A full revolution at 300 RPM (5Hz) is 200,000 cycles at 1MHz.
    // However, some tracks in Zone 0 can take up to 246,144 cycles.
    // We use 300,000 to be safe.
    for i in 0..300_000 {
        bus.tick_cycles(1); let byte_ready = bus.so_pending();

        let sync_active = (bus.via2.in_b & 0x80) == 0;

        if sync_active && !in_sync {
            sync_assertions += 1;
            in_sync = true;
        } else if !sync_active && in_sync {
            in_sync = false;
        }

        if byte_ready {
            byte_ready_pulses += 1;
            captured_bytes.push(bus.via2.in_a);
            
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
    let storage_g64 = MemoryStorage::new(&g64_data);
    let mut g64 = block_on(G64Image::from_storage(storage_g64)).expect("Failed to initialize G64Image");

    let mut g64_track_data = [0u8; 8192];
    // Track 18.0 is at half-track 36
    let g64_layout = block_on(g64.read_track(36, &mut g64_track_data)).expect("Failed to read G64 track 18");

    // 2. Load D64 Track 18 (HT 36)
    let d64_data = fs::read("tests/data/test.d64").expect("Failed to read D64");
    let mut storage_d64 = MemoryStorage::new(&d64_data);
    let mut d64 = D64Image::new(&mut storage_d64);
    
    let mut d64_track_data = [0u8; 8192];
    let d64_layout = block_on(d64.read_track(36, &mut d64_track_data)).expect("Failed to read D64 track 18");

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
