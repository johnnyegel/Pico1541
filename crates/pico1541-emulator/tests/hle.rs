use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_emulator::systembus::SystemBus;
use pico1541_emulator::hle::HleManager;

struct MockHardware {
    track: ActiveTrack,
}

impl IecBus for MockHardware {
    fn iec_host(&self) -> IecBusState { IecBusState::default() }
    fn iec_drive(&mut self, _: IecBusState) {}
}

impl HostInterface for MockHardware {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, _: DriveEvent) {}
}

#[test]
fn test_hle_intercept_command_patches_ram() {
    let mut hw = MockHardware { track: ActiveTrack::new() };
    let mut bus = SystemBus::<MockHardware>::new([0; 0x4000], 8);
    
    // 1. Simulate a "SYNTAX ERROR" state in RAM
    bus.ram[0x0255] = 1; // cmdwat: command waiting
    bus.ram[0x0200] = b'C'; // start of buffer
    bus.ram[0x026C] = 31; // erword: Syntax Error
    
    // 2. Perform HLE intercept
    HleManager::intercept_command(&mut bus);
    
    // 3. Verify RAM is patched to "00, OK"
    assert_eq!(bus.ram[0x0255], 0);
    assert_eq!(bus.ram[0x0200], 0);
    assert_eq!(bus.ram[0x026C], 0);
    
    // Verify error message buffer at $02D5
    assert_eq!(&bus.ram[0x02D5..0x02D5+6], b"00, OK");
}
