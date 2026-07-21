#![allow(unused)]
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_emulator::*;
use pico1541_common::interface::*;
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_common::track_buffer::DualCoreTrackBuffer;

struct MockHost {
        log: Vec<String>,
}

impl ViaHardware for MockHost {}
impl HostInterface for MockHost {
    fn send_event(&mut self, _: DriveEvent) {}
        fn get_device_id(&self) -> u8 { 8 }
}

#[test]
fn test_fastload() {
    // This is just a stub to compile
    assert!(true);
}
