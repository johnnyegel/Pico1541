use mos6502::memory::Bus;
use crate::via6522::Via6522;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
pub struct SystemBus {
    ram: [u8; RAM_SIZE],
    rom: [u8; ROM_SIZE],
    via1: Via6522,
    via2: Via6522,
}

impl SystemBus {
    pub fn new(rom: [u8; ROM_SIZE]) -> Self {
        Self {
            ram: [0; RAM_SIZE],
            rom,
            via1: Via6522::new(),
            via2: Via6522::new(),
        }
    }

    fn tick_cycle(&mut self) {
        // This method would be called on each CPU cycle to update timers,
        // handle interrupts, and manage handshakes. For now, it's a placeholder.
        self.via1.tick_cycle();
        self.via2.tick_cycle();
    }
}

impl Bus for SystemBus {
    fn get_byte(&mut self, addr: u16) -> u8 {
        self.tick_cycle(); // Update timers and handle interrupts on each access

        match addr {
            // 2KB RAM from $0000-$07FF
            0x0000..=0x07FF => self.ram[addr as usize],
            
            // VIA #1 at $1800-$180F, mirrored every 16 bytes
            0x1800..=0x1BFF => self.via1.read(addr),

            // VIA #2 at $1C00-$1C0F, mirrored every 16 bytes
            0x1C00..=0x1FFF => self.via2.read(addr),

            // 16KB ROM from $C000-$FFFF
            0xC000..=0xFFFF => self.rom[(addr - 0xC000) as usize],
            
            // Unmapped memory returns 0
            _ => 0,
        }
    }

    fn set_byte(&mut self, addr: u16, value: u8) {
        self.tick_cycle(); // Update timers and handle interrupts on each access

        match addr {
            // 2KB RAM from $0000-$07FF
            0x0000..=0x07FF => self.ram[addr as usize] = value,

            // VIA #1 at $1800-$180F
            0x1800..=0x1BFF => self.via1.write(addr, value),

            // VIA #2 at $1C00-$1C0F
            0x1C00..=0x1FFF => self.via2.write(addr, value),
            
            // ROM is not writable, so we do nothing.
            0xC000..=0xFFFF => { /* ROM is read-only */ }

            // Ignore writes to unmapped memory
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mos6502::{cpu::CPU, instruction::Nmos6502};

    fn create_test_pico_bus() -> SystemBus {
        let mut rom = [0; ROM_SIZE];
        // Put a simple instruction in ROM for testing: LDA #$42
        rom[0] = 0xA9; // LDA immediate
        rom[1] = 0x42; 
        SystemBus::new(rom)
    }

    #[test]
    fn test_ram_read_write() {
        let mut pico_bus = create_test_pico_bus();
        pico_bus.set_byte(0x0100, 0xAB);
        assert_eq!(pico_bus.get_byte(0x0100), 0xAB);
    }

    #[test]
    fn test_rom_read() {
        let mut pico_bus = create_test_pico_bus();
        assert_eq!(pico_bus.get_byte(0xC000), 0xA9);
        assert_eq!(pico_bus.get_byte(0xC001), 0x42);
    }

    #[test]
    fn test_rom_write_ignored() {
        let mut pico_bus = create_test_pico_bus();
        let initial_value = pico_bus.get_byte(0xC000);
        pico_bus.set_byte(0xC000, 0xFF);
        assert_eq!(pico_bus.get_byte(0xC000), initial_value);
    }
    
    #[test]
    fn cpu_can_read_from_rom() {
        let pico_bus = create_test_pico_bus();
        let mut cpu = CPU::new(pico_bus, Nmos6502::default());
        cpu.reset();

        // Execute one instruction (LDA #$42)
        // We need to tell the CPU where to start, which is read from the reset vector
        // Let's manually set PC to 0xC000 for a simple test
        cpu.registers.program_counter = 0xC000;
        cpu.single_step();

        assert_eq!(cpu.registers.accumulator, 0x42);
    }
}