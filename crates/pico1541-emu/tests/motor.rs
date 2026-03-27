use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::via6522::Via6522;
use pico1541_emu::drivemechanics::DriveMechanics;
use pico1541_emu::DriveHardware;
use core::sync::atomic::Ordering;

struct MockHardware {
    track: ActiveTrack,
}

impl DriveHardware for MockHardware {
    fn get_active_track(&self) -> &ActiveTrack { &self.track }
    fn notify_head_stepped(&mut self, _ht: u8) {}
    fn notify_track_dirty(&mut self) {}
    fn notify_led_changed(&mut self, _enabled: bool) {}
    fn notify_motor_changed(&mut self, _enabled: bool) {}
    fn get_iec_pins(&self) -> u8 { 0 }
    fn set_iec_pins(&mut self, _pins: u8) {}
}

#[test]
fn test_motor_delay() {
    let mut hardware = MockHardware { track: ActiveTrack::new() };
    hardware.track.disk_inserted.store(true, Ordering::Relaxed);
    hardware.track.bit_length = 8000 * 8;
    hardware.track.gcr_data[0] = 0xAA;
    hardware.track.gcr_data[1] = 0x55;

    let mut via2 = Via6522::default();
    let mut mechanics = DriveMechanics::new();

    // Motor OFF by default (ORB bit 2 is 1)
    via2.orb = 0x04; 
    
    // Tick 1000 times, should NOT deliver data
    for _ in 0..1000 {
        assert!(!mechanics.tick(&mut hardware, &mut via2));
    }

    // Turn Motor ON (ORB bit 2 is 0)
    via2.orb &= !0x04;
    
    // Tick 499,999 times, should NOT deliver data yet
    for _ in 0..499_999 {
        mechanics.tick(&mut hardware, &mut via2);
    }
    assert!(!mechanics.motor_spun_up);

    // One more tick, should spin up
    mechanics.tick(&mut hardware, &mut via2);
    assert!(mechanics.motor_spun_up);

    // Now it should eventually deliver data (every ~32-40 cycles)
    let mut delivered = false;
    for _ in 0..100 {
        if mechanics.tick(&mut hardware, &mut via2) {
            delivered = true;
            break;
        }
    }
    assert!(delivered, "Motor is spun up but not delivering data");

    // Turn Motor OFF
    via2.orb |= 0x04;
    
    // It should still deliver data during spin-down for 500,000 cycles
    assert!(mechanics.motor_spun_up);
    
    for _ in 0..499_999 {
        mechanics.tick(&mut hardware, &mut via2);
    }
    assert!(mechanics.motor_spun_up);
    
    // One more tick should spin down
    mechanics.tick(&mut hardware, &mut via2);
    assert!(!mechanics.motor_spun_up);
}
