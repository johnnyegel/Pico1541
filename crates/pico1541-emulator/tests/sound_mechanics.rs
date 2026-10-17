use pico1541_common::context::via::VIA2_MASK_MOTOR;
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::{DriveEvent, HostInterface};
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_common::sound::{get_and_clear_steps, is_motor_running};
use pico1541_common::track_buffer::DualCoreTrackBuffer;
use pico1541_emulator::drivemechanics::DriveMechanics;

struct DummyHw;
impl ViaHardware for DummyHw {}
impl HostInterface for DummyHw {
    fn send_event(&mut self, _: DriveEvent) {}
}

#[test]
fn test_mechanics_sound_triggers_and_bumping() {
    let _ = get_and_clear_steps();
    let track_data = Box::leak(Box::new([[0u8; 8192]; 2]));
    let hw_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new(track_data)));
    let (_manager, emulator) = hw_buffer.split();
    let surface = SoftwareMagneticSurface::new();
    let mut mechanics = DriveMechanics::new(surface, emulator);
    let mut hw = DummyHw;

    // 1. Motor turn ON
    mechanics.update_from_state(VIA2_MASK_MOTOR, 0, 0, &mut hw);
    assert!(is_motor_running());

    // 2. Motor turn OFF
    mechanics.update_from_state(0, VIA2_MASK_MOTOR, 100, &mut hw);
    assert!(!is_motor_running());

    // 3. Normal Step (Phase change 0 -> 1 = Step IN)
    let _ = get_and_clear_steps();
    mechanics.update_from_state(1, 0, 200, &mut hw);
    let (steps, bumped) = get_and_clear_steps();
    assert_eq!(steps, 1);
    assert!(!bumped);

    // 4. Head Bumping (Attempting to step OUT while at MIN_HALF_TRACK)
    // Step OUT repeatedly to reach MIN_HALF_TRACK
    // First step OUT from phase 1 -> 0 (delta = 3)
    mechanics.update_from_state(0, 1, 250, &mut hw);
    let mut phase = 0;
    for _ in 0..100 {
        // Step OUT: delta 3 (phase sequence: 0 -> 3 -> 2 -> 1 -> 0)
        let next_phase = (phase + 3) & 3;
        mechanics.update_from_state(next_phase, phase, 300, &mut hw);
        phase = next_phase;
    }
    let _ = get_and_clear_steps();

    // Now at min half-track, attempt another step OUT (0 -> 3)
    mechanics.update_from_state(3, 0, 500, &mut hw);
    let (steps, bumped) = get_and_clear_steps();
    assert_eq!(steps, 1);
    assert!(bumped, "Should flag head bumped when stepping below MIN_HALF_TRACK");

    // 5. Head Bumping at MAX_HALF_TRACK (Attempting to step IN while at MAX_HALF_TRACK)
    let mut phase = 3;
    for _ in 0..120 {
        // Step IN: delta 1 (phase sequence: 0 -> 1 -> 2 -> 3 -> 0)
        let next_phase = (phase + 1) & 3;
        mechanics.update_from_state(next_phase, phase, 600, &mut hw);
        phase = next_phase;
    }
    let _ = get_and_clear_steps();

    // Now at max half-track, attempt another step IN
    let next_phase = (phase + 1) & 3;
    mechanics.update_from_state(next_phase, phase, 700, &mut hw);
    let (steps, bumped) = get_and_clear_steps();
    assert_eq!(steps, 1);
    assert!(bumped, "Should flag head bumped when stepping above MAX_HALF_TRACK");
}
