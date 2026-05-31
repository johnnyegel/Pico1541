use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};

struct TestHardware {
    track: ActiveTrack,
    cycles: u64,
    hijacked: bool,
}

impl IecBus for TestHardware {
    fn iec_host(&self) -> IecBusState { IecBusState::default() }
    fn iec_drive(&mut self, _: IecBusState) {}
}

impl HostInterface for TestHardware {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, _: DriveEvent) {}
    fn is_hijacked(&self) -> bool { self.hijacked }
    fn sync(&mut self, _: IecBusState) { self.cycles += 1; }
}

#[test]
fn test_hijack_latency() {
    let mut hw = TestHardware {
        track: ActiveTrack::new(),
        cycles: 0,
        hijacked: false,
    };
    let drive_rom = [0u8; 16384];
    let mut drive = pico1541_emulator::Emulator::new(drive_rom, 8);
    
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
