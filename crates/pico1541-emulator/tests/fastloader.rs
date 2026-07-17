#![allow(unused)]
use pico1541_emulator::*;
use pico1541_common::interface::*;
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_common::track_buffer::DualCoreTrackBuffer;

struct MockHost {
    iec: IecBusState,
    log: Vec<String>,
}

impl IecBus for MockHost {
    fn iec_host(&self) -> IecBusState { self.iec }
    fn iec_drive(&mut self, state: IecBusState) {
        self.log.push(format!("DRIVE IEC: {:02X}", state.0));
    }
}
impl HostInterface for MockHost {
    fn send_event(&mut self, _: DriveEvent) {}
    fn sync(&mut self, _: IecBusState) {}
    fn get_device_id(&self) -> u8 { 8 }
}

#[test]
fn test_fastload() {
    // This is just a stub to compile
    assert!(true);
}
