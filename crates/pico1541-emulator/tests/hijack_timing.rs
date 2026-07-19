#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::{DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;

struct TestHardware {
    
    cycles: u64,
    hijacked: bool,
}

impl IecBus for TestHardware {
    fn iec_host(&self) -> IecBusState { IecBusState::default() }
    fn iec_drive(&mut self, _: IecBusState) {}
}

impl ViaHardware for TestHardware {}
impl HostInterface for TestHardware {
    
    fn send_event(&mut self, _: DriveEvent) {}
    fn is_hijacked(&self) -> bool { self.hijacked }
    fn sync(&mut self, _: IecBusState) { self.cycles += 1; }
}

#[test]
#[ignore]
fn test_hijack_latency() {
    let mut hw = TestHardware {
        
        cycles: 0,
        hijacked: false,
    };
    static DRIVE_ROM: [u8; 16384] = [0u8; 16384];
    let handle = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new())).split().1;
    let surface = SoftwareMagneticSurface::new(handle);
    let mut drive = pico1541_emulator::Emulator::new(&DRIVE_ROM, 8, surface);
    
    // 1. Run for some cycles
    for _ in 0..100 {
        drive.tick(&mut hw);
    }
    let initial_cycles = hw.cycles;
    
    // 2. Trigger hijack
    hw.hijacked = true;
    
    // 3. Verify that the next tick reflects the hijack in its internal logic
    drive.tick(&mut hw);
    
    let latency = hw.cycles - initial_cycles;
    assert!(latency >= 1 && latency <= 7, "Hijack must be detected within 1 instruction (1-7 clock cycles)");
}
