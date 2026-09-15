#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::{DriveEvent, HostInterface};
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_emulator::hle::HleManager;
use pico1541_emulator::systembus::SystemBus;

struct MockHardware {}

impl ViaHardware for MockHardware {}
impl HostInterface for MockHardware {
    fn send_event(&mut self, _: DriveEvent) {}
}

#[test]
fn test_hle_intercept_command_patches_ram() {
    static ROM: [u8; 0x4000] = [0; 0x4000];
    let handle = Box::leak(Box::new(
        pico1541_common::track_buffer::DualCoreTrackBuffer::new(),
    ))
    .split()
    .1;
    let surface = SoftwareMagneticSurface::new();
    let mechanics = pico1541_emulator::drivemechanics::DriveMechanics::new(surface, handle);
    let mut bus =
        SystemBus::<MockHardware, _>::new(&ROM, Box::leak(Box::new([0u8; 32768])), 8, mechanics);

    // 1. Simulate a "SYNTAX ERROR" state in RAM
    bus.memory[0x0255] = 1; // cmdwat: command waiting
    bus.memory[0x0200] = b'C'; // start of buffer
    bus.memory[0x026C] = 31; // erword: Syntax Error

    // 2. Perform HLE intercept
    HleManager::intercept_command(&mut bus);

    // 3. Verify RAM is patched to "00, OK"
    assert_eq!(bus.memory[0x0255], 0);
    assert_eq!(bus.memory[0x0200], 0);
    assert_eq!(bus.memory[0x026C], 0);

    // Verify error message buffer at $02D5
    assert_eq!(&bus.memory[0x02D5..0x02D5 + 6], b"00, OK");
}
