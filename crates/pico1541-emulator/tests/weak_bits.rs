use pico1541_common::magnetic_surface::{MagneticSurface, SoftwareMagneticSurface};
use pico1541_common::track_buffer::{
    default_speed_zone, DualCoreTrackBuffer, TrackIdentifier, TrackLayout, TRACK_SIZE,
};

#[test]
fn test_weak_bits_revolution_variance_and_no_false_sync() {
    let raw_buffers = Box::into_raw(Box::new([[0u8; TRACK_SIZE]; 2]));
    let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buffers)));
    let (manager, emulator) = dual_buffer.split();

    let track_len = 800;
    let weak_start_1 = 150;
    let weak_len_1 = 40;
    let weak_start_2 = 400;
    let weak_len_2 = 30;

    // Load track with known pattern outside weak ranges and 0x00 inside weak ranges
    let buf = manager.start_track_load();
    buf.fill(0x55);
    buf[weak_start_1..weak_start_1 + weak_len_1].fill(0x00);
    buf[weak_start_2..weak_start_2 + weak_len_2].fill(0x00);

    let speed_zone = default_speed_zone(0);
    manager.commit_loaded_track(
        TrackIdentifier::new(0, speed_zone),
        TrackLayout::Accurate,
        track_len,
    );

    let active_track = emulator.get_active_track_buffer();
    assert_eq!(active_track.weak_range_count, 2);
    assert_eq!(
        active_track.weak_ranges[0].unwrap().offset,
        weak_start_1 as u16
    );
    assert_eq!(
        active_track.weak_ranges[0].unwrap().length,
        weak_len_1 as u16
    );
    assert_eq!(
        active_track.weak_ranges[1].unwrap().offset,
        weak_start_2 as u16
    );
    assert_eq!(
        active_track.weak_ranges[1].unwrap().length,
        weak_len_2 as u16
    );

    // Verify initial replacement bytes in commit_loaded_track are non-zero and not 0xFF
    let committed_data = unsafe { *active_track.data };
    for idx in weak_start_1..weak_start_1 + weak_len_1 {
        let b = committed_data[idx];
        assert_ne!(b, 0x00, "Weak byte must not be 0x00 at idx {}", idx);
        assert_ne!(b, 0xFF, "Weak byte must not be 0xFF at idx {}", idx);
        assert_eq!(
            b & 0x81,
            0,
            "Weak byte at idx {} must have bit 7 and bit 0 cleared",
            idx
        );
    }

    let mut surface = SoftwareMagneticSurface::new();
    surface.set_stream_source(active_track, 0);
    surface.set_speed_zone(speed_zone);

    // Requirement 3d: Calling poll_revolution() mutates the weak bytes in the buffer
    surface.poll_revolution();
    let poll_rev1 = unsafe { *active_track.data };
    surface.poll_revolution();
    let poll_rev2 = unsafe { *active_track.data };

    // Bytes outside weak ranges must be 100% identical
    assert_eq!(
        &poll_rev1[0..weak_start_1],
        &poll_rev2[0..weak_start_1],
        "Bytes before range 1 must be identical after poll_revolution"
    );
    assert_eq!(
        &poll_rev1[weak_start_1 + weak_len_1..weak_start_2],
        &poll_rev2[weak_start_1 + weak_len_1..weak_start_2],
        "Bytes between ranges must be identical after poll_revolution"
    );
    assert_eq!(
        &poll_rev1[weak_start_2 + weak_len_2..track_len],
        &poll_rev2[weak_start_2 + weak_len_2..track_len],
        "Bytes after range 2 must be identical after poll_revolution"
    );

    // Bytes inside weak ranges must differ between successive poll_revolution calls
    assert_ne!(
        &poll_rev1[weak_start_1..weak_start_1 + weak_len_1],
        &poll_rev2[weak_start_1..weak_start_1 + weak_len_1],
        "Range 1 bytes must mutate across poll_revolution calls"
    );
    assert_ne!(
        &poll_rev1[weak_start_2..weak_start_2 + weak_len_2],
        &poll_rev2[weak_start_2..weak_start_2 + weak_len_2],
        "Range 2 bytes must mutate across poll_revolution calls"
    );

    // Reset surface stream index to 0 for revolution test
    surface.set_stream_source(active_track, 0);

    // Requirement 3a, 3b, 3c: Stream two complete revolutions using try_read_byte
    let mut rev1_bytes = Vec::with_capacity(track_len);
    for _ in 0..track_len {
        let byte_opt = surface.try_read_byte(26);
        assert!(byte_opt.is_some(), "Expected byte assembled from surface");
        rev1_bytes.push(byte_opt.unwrap());
        // Requirement 3c: Weak bytes must never trigger false SYNC
        assert!(
            !surface.is_sync_active(),
            "False SYNC detected during revolution 1"
        );
    }

    // Now stream revolution 2 (the wrap-around from byte track_len back to 0 will mutate weak ranges)
    let mut rev2_bytes = Vec::with_capacity(track_len);
    for _ in 0..track_len {
        let byte_opt = surface.try_read_byte(26);
        assert!(byte_opt.is_some(), "Expected byte assembled from surface");
        rev2_bytes.push(byte_opt.unwrap());
        // Requirement 3c: Weak bytes must never trigger false SYNC
        assert!(
            !surface.is_sync_active(),
            "False SYNC detected during revolution 2"
        );
    }

    // Requirement 3a: Data outside the weak range is 100% identical between revolution 1 and revolution 2
    assert_eq!(
        &rev1_bytes[0..weak_start_1],
        &rev2_bytes[0..weak_start_1],
        "Normal data before range 1 must be identical across revolutions"
    );
    assert_eq!(
        &rev1_bytes[weak_start_1 + weak_len_1..weak_start_2],
        &rev2_bytes[weak_start_1 + weak_len_1..weak_start_2],
        "Normal data between ranges must be identical across revolutions"
    );
    assert_eq!(
        &rev1_bytes[weak_start_2 + weak_len_2..track_len],
        &rev2_bytes[weak_start_2 + weak_len_2..track_len],
        "Normal data after range 2 must be identical across revolutions"
    );

    // Requirement 3b: Data inside the weak range differs between revolution 1 and revolution 2
    assert_ne!(
        &rev1_bytes[weak_start_1..weak_start_1 + weak_len_1],
        &rev2_bytes[weak_start_1..weak_start_1 + weak_len_1],
        "Weak range 1 data must differ between revolution 1 and revolution 2"
    );
    assert_ne!(
        &rev1_bytes[weak_start_2..weak_start_2 + weak_len_2],
        &rev2_bytes[weak_start_2..weak_start_2 + weak_len_2],
        "Weak range 2 data must differ between revolution 1 and revolution 2"
    );
}

#[test]
fn test_weak_bits_boundary_sync_immunity() {
    let raw_buffers = Box::into_raw(Box::new([[0u8; TRACK_SIZE]; 2]));
    let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buffers)));
    let (manager, emulator) = dual_buffer.split();

    let track_len = 100;
    let buf = manager.start_track_load();
    // Fill with 0b0100_1111 (0x4F: ends with four 1s) and 0b1111_0010 (0xF2: starts with four 1s)
    // to test boundary conditions next to weak ranges.
    buf.fill(0x55);
    buf[10] = 0x4F;
    buf[11..20].fill(0x00); // 9 weak bytes
    buf[20] = 0xF2;

    manager.commit_loaded_track(
        TrackIdentifier::new(0, 3),
        TrackLayout::Accurate,
        track_len,
    );

    let active_track = emulator.get_active_track_buffer();
    let mut surface = SoftwareMagneticSurface::new();
    surface.set_stream_source(active_track, 0);
    surface.set_speed_zone(3);

    for _ in 0..track_len * 3 {
        let _ = surface.try_read_byte(26);
        assert!(
            !surface.is_sync_active(),
            "Boundary conditions must never trigger false SYNC"
        );
    }
}

#[test]
fn test_load_next_track_resets_weak_ranges() {
    let raw_buffers = Box::into_raw(Box::new([[0u8; TRACK_SIZE]; 2]));
    let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(raw_buffers)));
    let (manager, emulator) = dual_buffer.split();

    // First, commit a track that has weak ranges
    let buf = manager.start_track_load();
    buf[..100].fill(0x55);
    buf[10..20].fill(0x00);
    manager.commit_loaded_track(TrackIdentifier::new(1, 3), TrackLayout::Accurate, 100);

    let active_track = emulator.get_active_track_buffer();
    assert_eq!(active_track.weak_range_count, 1);
    assert!(active_track.weak_ranges[0].is_some());

    // Switch back buffer with weak ranges to active, so the other buffer can be loaded
    let buf2 = manager.start_track_load();
    buf2[..100].fill(0x55);
    buf2[15..30].fill(0x00);
    manager.commit_loaded_track(TrackIdentifier::new(2, 3), TrackLayout::Accurate, 100);

    // Now call load_next_track on manager: it must reset weak_ranges and weak_range_count
    manager.load_next_track(|data, layout, id| {
        data[..100].fill(0x55);
        *layout = TrackLayout::Accurate;
        *id = TrackIdentifier::new(3, 3);
    });

    let new_active_track = emulator.get_active_track_buffer();
    assert_eq!(
        new_active_track.weak_range_count, 0,
        "load_next_track must reset weak_range_count to 0"
    );
    for r in &new_active_track.weak_ranges {
        assert_eq!(*r, None, "load_next_track must reset weak_ranges to None");
    }
}

