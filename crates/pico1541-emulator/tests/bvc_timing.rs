use pico1541_emulator::*;
use pico1541_common::interface::*;
use pico1541_common::magnetic_surface::{SoftwareMagneticSurface};
use pico1541_common::context::ViaHardware;
use pico1541_common::track_buffer::{DualCoreTrackBuffer, TrackLayout};

struct MockHardware;
impl ViaHardware for MockHardware {}
impl HostInterface for MockHardware {
    fn send_event(&mut self, _: DriveEvent) {}
    fn get_device_id(&self) -> u8 { 8 }
}

static DUMMY_ROM: [u8; 16384] = [0; 16384];

#[test]
fn test_bvc_so_timing() {
    let mut hw = MockHardware;
    let hw_buffer = Box::leak(Box::new(DualCoreTrackBuffer::new()));
    let (manager, emulator) = hw_buffer.split();
    let surface = SoftwareMagneticSurface::new();
    
    let mechanics = pico1541_emulator::drivemechanics::DriveMechanics::new(surface, emulator);
    let mut emu = Emulator::new(&DUMMY_ROM, Box::leak(Box::new([0u8; 32768])), 8, mechanics);

    // Setup a GCR track with a single byte for testing
    {
        let inactive = manager.get_dormant_buffer_mut();
        inactive.data[0] = 0x55;
        inactive.data[1] = 0xAA;
        inactive.data[2] = 0x55;
        inactive.data[3] = 0xAA;
        inactive.data[4] = 0x55;
    }
    manager.commit_loaded_track(pico1541_common::track_buffer::TrackIdentifier::new(0, pico1541_common::track_buffer::default_speed_zone(0)), TrackLayout::Accurate, 8000);

    // Write a tiny program to RAM:
    // $0000: BVC $0000 (50 FE)
    // $0002: CLV       (B8)
    // $0003: LDA $1C01 (AD 01 1C)
    // $0006: STA $0700 (8D 00 07)
    // $0009: BRK       (00)
    emu.reset(&mut hw);

    emu.systembus.memory[0x0000] = 0x50; emu.systembus.memory[0x0001] = 0xFE;
    emu.systembus.memory[0x0002] = 0xB8;
    emu.systembus.memory[0x0003] = 0xAD; emu.systembus.memory[0x0004] = 0x01; emu.systembus.memory[0x0005] = 0x1C;
    emu.systembus.memory[0x0006] = 0x8D; emu.systembus.memory[0x0007] = 0x00; emu.systembus.memory[0x0008] = 0x07;
    emu.systembus.memory[0x0009] = 0x00;

    emu.cpu.pc = 0x0000;

    // Enable motor and SO (PCR = 0xEE)
    emu.systembus.via2.write(0x1C02, 0xFF); // DDRB = output
    emu.systembus.via2.write(0x1C0C, 0xEE); // PCR = 0xEE (SO enable)
    emu.systembus.via2.write(0x1C00, 0x04); // Motor ON
    
    // update state
    let effective_orb = emu.systembus.via2.get_pb_out();
    emu.systembus.mechanics.update_state(effective_orb, &mut hw, 0);
    
    emu.systembus.mechanics.force_state_for_testing(0, 3, 0);

    // Run until BRK (PC >= 0x0009)
    let mut finished = false;
    for _ in 0..1000 {
        emu.step(&mut hw);
        let pc = emu.cpu.pc;
        let a = emu.cpu.a;
        println!("PC: {:04X}, A: {:02X}, in_a: {:02X}", pc, a, emu.systembus.via2.in_a);

        if emu.cpu.pc >= 0x0009 {
            finished = true;
            break;
        }
    }

    assert!(finished, "CPU hung in BVC loop! Timeout.");
    assert_eq!(emu.systembus.memory[0x0700], 0x55, "Should have read 0x55 from disk");
}
