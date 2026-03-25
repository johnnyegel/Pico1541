use racc6502::cpu::{CPU, CycleState};
use racc6502::bus::Bus;
use racc6502::instructions::{VECTOR_RESET, VECTOR_IRQ, VECTOR_NMI};

struct MockBus {
    memory: [u8; 65536],
    pub access_count: usize,
    pub irq_pending: bool,
    pub nmi_pending: bool,
    pub so_pending: bool,
}

impl MockBus {
    fn new() -> Self {
        Self { 
            memory: [0; 65536], 
            access_count: 0,
            irq_pending: false,
            nmi_pending: false,
            so_pending: false,
        }
    }
}

impl Bus for MockBus {
    fn read(&mut self, addr: u16) -> u8 { self.access_count += 1; self.memory[addr as usize] }
    fn write(&mut self, addr: u16, value: u8) { self.access_count += 1; self.memory[addr as usize] = value; }
    fn irq_pending(&self) -> bool { self.irq_pending }
    fn nmi_pending(&self) -> bool { self.nmi_pending }
    fn so_pending(&self) -> bool { self.so_pending }
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

#[test]
fn test_data_movement() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    
    bus.memory[0x8000] = 0xAD; // LDA Abs $1234
    bus.memory[0x8001] = 0x34;
    bus.memory[0x8002] = 0x12;
    bus.memory[0x1234] = 0x42;
    bus.memory[0x8003] = 0xA2; // LDX Imm $0F
    bus.memory[0x8004] = 0x0F;
    bus.memory[0x8005] = 0x85; // STA ZP $10
    bus.memory[0x8006] = 0x10;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    for _ in 0..4 { cpu.tick(); } // LDA Abs
    assert_eq!(cpu.a, 0x42);
    for _ in 0..2 { cpu.tick(); } // LDX Imm
    assert_eq!(cpu.x, 0x0F);
    for _ in 0..3 { cpu.tick(); } // STA ZP
    assert_eq!(cpu.bus.memory[0x10], 0x42);
}

#[test]
fn test_arithmetic() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    
    bus.memory[0x8000] = 0x38; // SEC
    bus.memory[0x8001] = 0x69; // ADC Imm $05
    bus.memory[0x8002] = 0x05;
    bus.memory[0x8003] = 0xF8; // SED
    bus.memory[0x8004] = 0xA9; // LDA Imm $15
    bus.memory[0x8005] = 0x15;
    bus.memory[0x8006] = 0x38; // SEC
    bus.memory[0x8007] = 0xE9; // SBC Imm $06
    bus.memory[0x8008] = 0x06;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    cpu.a = 0x10;
    for _ in 0..2 { cpu.tick(); } // SEC
    assert_eq!(cpu.p & 0x01, 0x01);
    for _ in 0..2 { cpu.tick(); } // ADC Imm
    assert_eq!(cpu.a, 0x16);
    for _ in 0..2 { cpu.tick(); } // SED
    assert_eq!(cpu.p & 0x08, 0x08);
    for _ in 0..2 { cpu.tick(); } // LDA Imm
    for _ in 0..2 { cpu.tick(); } // SEC
    for _ in 0..2 { cpu.tick(); } // SBC Imm (Decimal: 0x15 - 0x06 = 0x09)
    assert_eq!(cpu.a, 0x09);
}

#[test]
fn test_logical() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    
    bus.memory[0x8000] = 0x29; // AND Imm $0F
    bus.memory[0x8001] = 0x0F;
    bus.memory[0x8002] = 0x2C; // BIT Abs $1234
    bus.memory[0x8003] = 0x34;
    bus.memory[0x8004] = 0x12;
    bus.memory[0x1234] = 0xC0;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    cpu.a = 0xFF;
    for _ in 0..2 { cpu.tick(); } // AND Imm
    assert_eq!(cpu.a, 0x0F);
    cpu.a = 0x01;
    for _ in 0..4 { cpu.tick(); } // BIT Abs
    assert_eq!(cpu.p & 0x02, 0x02); // Zero set because (0x01 & 0xC0) == 0
    assert_eq!(cpu.p & 0xC0, 0xC0); // N and V set from memory $C0
}

#[test]
fn test_rmw() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[0x8000] = 0x06; // ASL ZP $10
    bus.memory[0x8001] = 0x10;
    bus.memory[0x0010] = 0x80;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    for _ in 0..5 { cpu.tick(); } // ASL ZP
    assert_eq!(cpu.bus.memory[0x10], 0x00);
    assert_eq!(cpu.p & 0x01, 0x01); // Carry set
    assert_eq!(cpu.p & 0x02, 0x02); // Zero set
}

#[test]
fn test_stack() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[0x8000] = 0x48; // PHA
    bus.memory[0x8001] = 0xA9; // LDA Imm $00
    bus.memory[0x8002] = 0x00;
    bus.memory[0x8003] = 0x68; // PLA

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    cpu.a = 0x42;
    let s_before = cpu.s;
    for _ in 0..3 { cpu.tick(); } // PHA
    assert_eq!(cpu.s, s_before.wrapping_sub(1));
    for _ in 0..2 { cpu.tick(); } // LDA Imm
    assert_eq!(cpu.a, 0x00);
    for _ in 0..4 { cpu.tick(); } // PLA
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.s, s_before);
}

#[test]
fn test_control_flow() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[0x8000] = 0x20; // JSR $9000
    bus.memory[0x8001] = 0x00;
    bus.memory[0x8002] = 0x90;
    bus.memory[0x9000] = 0x60; // RTS
    bus.memory[0x8003] = 0xD0; // BNE +2 (Not taken)
    bus.memory[0x8004] = 0x02;
    bus.memory[0x8005] = 0xA9; // LDA Imm $01
    bus.memory[0x8006] = 0x01;
    bus.memory[0x8007] = 0xD0; // BNE -4 (Taken)
    bus.memory[0x8008] = 0xFC;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    for _ in 0..6 { cpu.tick(); } // JSR
    assert_eq!(cpu.pc, 0x9000);
    for _ in 0..6 { cpu.tick(); } // RTS
    assert_eq!(cpu.pc, 0x8003);
    
    cpu.p |= 0x02; // Set Zero
    let cycles_before = cpu.bus.access_count;
    for _ in 0..2 { cpu.tick(); } // BNE Not taken
    assert_eq!(cpu.pc, 0x8005);
    assert_eq!(cpu.bus.access_count - cycles_before, 2);
    
    for _ in 0..2 { cpu.tick(); } // LDA Imm (Clears Zero)
    assert_eq!(cpu.p & 0x02, 0);
    let cycles_before = cpu.bus.access_count;
    for _ in 0..3 { cpu.tick(); } // BNE Taken (same page)
    assert_eq!(cpu.pc, 0x8005);
    assert_eq!(cpu.bus.access_count - cycles_before, 3);
}

#[test]
fn test_interrupt_brk() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[VECTOR_IRQ as usize] = 0x00;
    bus.memory[(VECTOR_IRQ + 1) as usize] = 0x90;
    bus.memory[0x8000] = 0x00; // BRK
    bus.memory[0x8001] = 0xEA;

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    let p_before = cpu.p;
    for _ in 0..7 { cpu.tick(); } // BRK
    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(cpu.bus.memory[0x01FD], 0x80); // PCH
    assert_eq!(cpu.bus.memory[0x01FC], 0x02); // PCL
    assert_eq!(cpu.bus.memory[0x01FB], p_before | 0x30); // P with B and Bit 5
}

#[test]
fn test_signal_so() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[0x8000] = 0xEA; // NOP

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    assert_eq!(cpu.p & 0x40, 0); // V should be clear
    
    cpu.bus.so_pending = true;
    cpu.tick(); // First cycle of NOP fetch
    assert_eq!(cpu.p & 0x40, 0x40); // V should be set
}

#[test]
fn test_signal_nmi() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[VECTOR_NMI as usize] = 0x00;
    bus.memory[(VECTOR_NMI + 1) as usize] = 0x90;
    bus.memory[0x8000] = 0xEA; // NOP

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    cpu.bus.nmi_pending = true;
    let p_before = cpu.p;
    
    // NMI is sampled at the start of tick_fetch.
    for _ in 0..7 { cpu.tick(); }
    
    assert_eq!(cpu.pc, 0x9000);
    assert_eq!(cpu.bus.memory[0x01FD], 0x80); // PCH (points to NOP at 0x8000)
    assert_eq!(cpu.bus.memory[0x01FC], 0x00); // PCL
    assert_eq!(cpu.bus.memory[0x01FB], (p_before & 0xCF) | 0x20); 
    assert_eq!(cpu.p & 0x04, 0x04); // Interrupt Disable should be set
}

#[test]
fn test_signal_irq() {
    let mut bus = MockBus::new();
    bus.memory[VECTOR_RESET as usize] = 0x00;
    bus.memory[(VECTOR_RESET + 1) as usize] = 0x80;
    bus.memory[VECTOR_IRQ as usize] = 0x00;
    bus.memory[(VECTOR_IRQ + 1) as usize] = 0xA0;
    bus.memory[0x8000] = 0xEA; // NOP
    bus.memory[0x8001] = 0xEA; // NOP

    let mut cpu = CPU::new(bus);
    for _ in 0..8 { cpu.tick(); }
    
    // 1. IRQ ignored when I=1
    cpu.p |= 0x04; // Set I
    cpu.bus.irq_pending = true;
    cpu.tick(); // Cycle 1 of NOP: fetches EA
    assert_eq!(cpu.ir, 0xEA);
    cpu.tick(); // Cycle 2 of NOP
    assert_ne!(cpu.pc, 0xA000); // Should still be at 0x8001
    
    // 2. IRQ taken when I=0
    cpu.p &= !0x04; // Clear I
    
    let pc_before = cpu.pc;
    let p_before = cpu.p;
    for _ in 0..7 { cpu.tick(); } // IRQ sequence
    
    assert_eq!(cpu.pc, 0xA000);
    assert_eq!(cpu.bus.memory[0x01FD], (pc_before >> 8) as u8);
    assert_eq!(cpu.bus.memory[0x01FC], (pc_before & 0xFF) as u8);
    assert_eq!(cpu.bus.memory[0x01FB], (p_before & 0xCF) | 0x20);
}
