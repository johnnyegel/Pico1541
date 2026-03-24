use mos6502::memory::Bus;
use crate::via6522::Via6522;
use crate::DriveHardware;
use crate::drivemechanics::DriveMechanics;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
pub struct SystemBus<H: DriveHardware> {
    ram: [u8; RAM_SIZE],
    rom: [u8; ROM_SIZE],
    pub via1: Via6522,
    pub via2: Via6522,
    pub hardware: H,
    pub mechanics: DriveMechanics,
    pub so_pending: bool,
}

impl<H: DriveHardware> SystemBus<H> {
    pub fn new(rom: [u8; ROM_SIZE], hardware: H) -> Self {
        Self {
            ram: [0; RAM_SIZE],
            rom,
            via1: Via6522::new(),
            via2: Via6522::new(),
            hardware,
            mechanics: DriveMechanics::new(),
            so_pending: false,
        }
    }

    fn tick_cycle(&mut self) {
        // Handle physical drive mechanics (rotation, stepping, density)
        if self.mechanics.tick(&mut self.hardware, &mut self.via2) {
            self.so_pending = true;
        }

        // Bridge IEC pins to VIA1 Port B
        self.via1.in_b = self.hardware.get_iec_pins();
        self.hardware.set_iec_pins(self.via1.get_port_b());

        self.via1.tick_cycle();
        self.via2.tick_cycle();
    }
}

impl<H: DriveHardware> Bus for SystemBus<H> {
    fn get_byte(&mut self, addr: u16) -> u8 {
        self.tick_cycle();

        match addr {
            0x0000..=0x07FF => self.ram[addr as usize],
            0x1800..=0x1BFF => self.via1.read(addr),
            0x1C00..=0x1FFF => self.via2.read(addr),
            0xC000..=0xFFFF => self.rom[(addr - 0xC000) as usize],
            _ => 0,
        }
    }

    fn set_byte(&mut self, addr: u16, value: u8) {
        self.tick_cycle();

        match addr {
            0x0000..=0x07FF => self.ram[addr as usize] = value,
            0x1800..=0x1BFF => self.via1.write(addr, value),
            0x1C00..=0x1FFF => self.via2.write(addr, value),
            0xC000..=0xFFFF => { /* ROM is read-only */ }
            _ => {}
        }
    }

    fn irq_pending(&mut self) -> bool {
        self.via1.irq || self.via2.irq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mos6502::{cpu::CPU, instruction::Nmos6502};
    use crate::activetrack::ActiveTrack;
    use crate::Command;

    struct MockHardware {
        track: ActiveTrack,
    }

    impl DriveHardware for MockHardware {
        fn get_active_track(&self) -> &ActiveTrack { &self.track }
        fn notify_head_stepped(&mut self, _current_half_track: u8) {}
        fn notify_track_dirty(&mut self) {}
        fn get_iec_pins(&self) -> u8 { 0xFF }
        fn set_iec_pins(&mut self, _pins: u8) {}
    }

    #[test]
    fn test_ram_read_write() {
        let hardware = MockHardware { track: ActiveTrack::new() };
        let mut pico_bus = SystemBus::new([0; ROM_SIZE], hardware);
        
        pico_bus.set_byte(0x0100, 0xAB);
        assert_eq!(pico_bus.get_byte(0x0100), 0xAB);
    }

    #[test]
    fn cpu_can_read_from_rom() {
        let mut rom = [0; ROM_SIZE];
        rom[0] = 0xA9; // LDA immediate
        rom[1] = 0x42; 
        
        let hardware = MockHardware { track: ActiveTrack::new() };
        let pico_bus = SystemBus::new(rom, hardware);
        
        let mut cpu = CPU::new(pico_bus, Nmos6502::default());
        cpu.reset();

        cpu.registers.program_counter = 0xC000;
        cpu.single_step();

        assert_eq!(cpu.registers.accumulator, 0x42);
    }
}
