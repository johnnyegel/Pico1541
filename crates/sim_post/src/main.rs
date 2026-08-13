#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_common::track_buffer::{DualCoreTrackBuffer, EmulatorHandle};
use pico1541_common::interface::{DriveEvent, HostInterface, IecBusState, IecBus};
use pico1541_emulator::Emulator;

struct DummyHost {}
impl IecBus for DummyHost {
    fn iec_host(&self) -> IecBusState { IecBusState::default() }
    fn iec_drive(&mut self, _state: IecBusState) {}
}
impl ViaHardware for DummyHost {}
impl HostInterface for DummyHost {
    fn send_event(&mut self, _event: DriveEvent) {}
    fn track_changed(&mut self, _track: u8) {}
}

const ROM: [u8; 16384] = *include_bytes!("../../../flash-fs/roms/1541-II.251968-03.bin");

fn main() {
    let dual_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new()));
    let (manager, emulator_handle) = dual_buffer.split();
    manager.set_write_protected(true);

    let surface = SoftwareMagneticSurface::new();
    let mechanics = pico1541_emulator::drivemechanics::DriveMechanics::new(surface, emulator_handle);
    let mut drive = Emulator::new(&ROM, Box::leak(Box::new([0u8; 32768])), 8, mechanics);
    let mut bridge = DummyHost {};
    for _ in 0..4000000 {
        drive.tick(&mut bridge);
    }
    let pc = drive.cpu.pc;
    println!("PC after 4M cycles (WP=true): {:04X}", pc);
}

#[test]
fn test_size() {
    println!("SIZE: {}", core::mem::size_of::<pico1541_common::formats::G64Image<()>>());
}
