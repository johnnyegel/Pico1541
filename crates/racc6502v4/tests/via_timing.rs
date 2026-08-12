use racc6502v4::{cpu::CPU, bus::Bus};
use std::vec::Vec;

#[derive(Debug, PartialEq, Eq)]
enum Access {
    Read { addr: u16, cycle: u32 },
    Write { addr: u16, value: u8, cycle: u32 },
}

struct CycleTrackingBus {
    mem: [u8; 65536],
    accesses: Vec<Access>,
}

impl CycleTrackingBus {
    fn new() -> Self {
        Self {
            mem: [0; 65536],
            accesses: Vec::new(),
        }
    }
}

impl Bus for CycleTrackingBus {
    type Ctx = ();
    
    fn read(&mut self, _ctx: &mut (), addr: u16, cycle: u32) -> u8 {
        self.accesses.push(Access::Read { addr, cycle });
        self.mem[addr as usize]
    }
    
    fn write(&mut self, _ctx: &mut (), addr: u16, value: u8, cycle: u32) {
        self.accesses.push(Access::Write { addr, value, cycle });
        self.mem[addr as usize] = value;
    }
    
    fn get_instruction_ptr(&self, addr: u16) -> *const u8 {
        &self.mem[addr as usize]
    }
}

#[test]
fn test_via_instruction_timing() {
    // Tests for specific absolute memory access instructions 
    // commonly used for accessing the VIA chip (e.g. at $1800)
    // to ensure the exact cycle of bus access is correctly reported.
    
    // Test LDA $1800 (opcode $AD)
    {
        let mut bus = CycleTrackingBus::new();
        let mut cpu = CPU::new();
        bus.mem[0] = 0xAD; bus.mem[1] = 0x00; bus.mem[2] = 0x18;
        cpu.pc = 0;
        
        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cycles, 4, "LDA absolute should take 4 cycles");
        // LDA absolute bus access happens on the 4th cycle (relative cycle 3)
        assert_eq!(bus.accesses, vec![
            Access::Read { addr: 0x1800, cycle: 3 }
        ]);
    }
    
    // Test STA $1800 (opcode $8D)
    {
        let mut bus = CycleTrackingBus::new();
        let mut cpu = CPU::new();
        bus.mem[0] = 0x8D; bus.mem[1] = 0x00; bus.mem[2] = 0x18;
        cpu.pc = 0;
        cpu.a = 0x42;
        
        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cycles, 4, "STA absolute should take 4 cycles");
        assert_eq!(bus.accesses, vec![
            Access::Write { addr: 0x1800, value: 0x42, cycle: 3 }
        ]);
    }
    
    // Test EOR $1800 (opcode $4D)
    {
        let mut bus = CycleTrackingBus::new();
        let mut cpu = CPU::new();
        bus.mem[0] = 0x4D; bus.mem[1] = 0x00; bus.mem[2] = 0x18;
        bus.mem[0x1800] = 0x55;
        cpu.pc = 0;
        cpu.a = 0xAA;
        
        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cycles, 4, "EOR absolute should take 4 cycles");
        assert_eq!(bus.accesses, vec![
            Access::Read { addr: 0x1800, cycle: 3 }
        ]);
        assert_eq!(cpu.a, 0xFF, "EOR result incorrect");
    }
    
    // Test BIT $1800 (opcode $2C)
    {
        let mut bus = CycleTrackingBus::new();
        let mut cpu = CPU::new();
        bus.mem[0] = 0x2C; bus.mem[1] = 0x00; bus.mem[2] = 0x18;
        bus.mem[0x1800] = 0x80;
        cpu.pc = 0;
        cpu.a = 0xFF;
        
        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cycles, 4, "BIT absolute should take 4 cycles");
        assert_eq!(bus.accesses, vec![
            Access::Read { addr: 0x1800, cycle: 3 }
        ]);
        // BIT sets N flag to bit 7 (which is 1)
        assert_eq!(cpu.p & 0x80, 0x80, "BIT flag N not set");
    }
}
