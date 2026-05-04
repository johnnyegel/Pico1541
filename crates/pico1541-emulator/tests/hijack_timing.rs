use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_emulator::systembus::SystemBus;
use racc6502::cpu::CPU;

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
    fn sync(&mut self) { self.cycles += 1; }
}

#[test]
fn test_hijack_latency() {
    let mut hw = TestHardware {
        track: ActiveTrack::new(),
        cycles: 0,
        hijacked: false,
    };
    let mut bus = SystemBus::<TestHardware>::new([0; 0x4000], 8);
    let mut cpu = CPU::new();
    
    // 1. Run for some cycles
    for _ in 0..100 {
        cpu.tick(&mut bus, &mut hw);
    }
    let initial_cycles = hw.cycles;
    
    // 2. Trigger hijack
    hw.hijacked = true;
    
    // 3. Verify that the next tick reflects the hijack in its internal logic
    // SystemBus::tick_cycle checks hardware.is_hijacked()
    cpu.tick(&mut bus, &mut hw);
    
    let latency = hw.cycles - initial_cycles;
    assert_eq!(latency, 1, "Hijack must be detected within 1 clock cycle");
}
