use racc6502::cpu::{CPU, CycleState};
use racc6502::bus::Bus;
use racc6502::instructions::VECTOR_RESET;

struct MockBus {
    memory: [u8; 65536],
    pub access_count: usize,
}

impl MockBus {
    fn new() -> Self {
        Self { memory: [0; 65536], access_count: 0 }
    }
}

impl Bus for MockBus {
    fn read(&mut self, addr: u16) -> u8 { self.access_count += 1; self.memory[addr as usize] }
    fn write(&mut self, addr: u16, value: u8) { self.access_count += 1; self.memory[addr as usize] = value; }
}

#[test]
fn test_reset_sequence() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x34;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x12;
    
    // LDA Imm $42
    bus.memory[0x1234] = 0xA9;
    bus.memory[0x1235] = 0x42;

    let mut cpu = CPU::new(bus);
    
    // Initial state is Reset step 0
    // We need 8 ticks to finish reset
    for _ in 0..8 { cpu.tick(); }
    
    assert_eq!(cpu.pc, 0x1234);
    assert_eq!(cpu.state, CycleState::FetchOpcode);
    
    // Next tick fetches LDA
    cpu.tick();
    assert_eq!(cpu.ir, 0xA9);
    
    // Next tick executes LDA Imm
    cpu.tick();
    assert_eq!(cpu.a, 0x42);
}
