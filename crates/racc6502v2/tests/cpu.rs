use racc6502v2::{CPU, Bus};

struct TestBus {
    mem: [u8; 65536],
}

impl Bus for TestBus {
    type Ctx = ();
    fn read(&mut self, addr: u16, _ctx: &mut ()) -> u8 {
        self.mem[addr as usize]
    }
    fn write(&mut self, addr: u16, value: u8, _ctx: &mut ()) {
        self.mem[addr as usize] = value;
    }
}

#[test]
fn test_adc_imm() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // ADC #$10 ($69 $10)
    bus.mem[0] = 0x69;
    bus.mem[1] = 0x10;
    cpu.pc = 0;
    cpu.a = 0x20;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch 0x69
    assert_eq!(cpu.opcode, 0x69);
    assert_eq!(cpu.cycle, 1);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch 0x10 and ADC
    assert_eq!(cpu.a, 0x30);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_sta_abs() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // STA $1234 ($8D $34 $12)
    bus.mem[0] = 0x8D;
    bus.mem[1] = 0x34;
    bus.mem[2] = 0x12;
    cpu.pc = 0;
    cpu.a = 0x42;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch 0x8D
    assert_eq!(cpu.opcode, 0x8D);
    assert_eq!(cpu.cycle, 1);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $34
    assert_eq!(cpu.effective_addr, 0x34);
    assert_eq!(cpu.cycle, 2);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Fetch $12
    assert_eq!(cpu.effective_addr, 0x1234);
    assert_eq!(cpu.cycle, 3);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 3: Write $42 to $1234
    assert_eq!(bus.mem[0x1234], 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_reset() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    cpu.reset();
    
    // Reset vector at $FFFC = $00, $FFFD = $C0 ($C000)
    bus.mem[0xFFFC] = 0x00;
    bus.mem[0xFFFD] = 0xC0;
    
    // CPU starts with pending_interrupt = Reset, opcode = OP_RESET, cycle = 0
    cpu.tick(&mut bus, &mut ()); // Cycle 0: fetch_opcode handles Reset
    assert_eq!(cpu.cycle, 1);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 1
    cpu.tick(&mut bus, &mut ()); // Cycle 2
    cpu.tick(&mut bus, &mut ()); // Cycle 3
    cpu.tick(&mut bus, &mut ()); // Cycle 4
    cpu.tick(&mut bus, &mut ()); // Cycle 5: Fetch vector LO
    assert_eq!(cpu.pc & 0xFF, 0x00);
    
    cpu.tick(&mut bus, &mut ()); // Cycle 6: Fetch vector HI
    assert_eq!(cpu.pc, 0xC000);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_lda_zp() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // LDA $20 ($A5 $20)
    bus.mem[0] = 0xA5;
    bus.mem[1] = 0x20;
    bus.mem[0x20] = 0x42;
    cpu.pc = 0;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $A5
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $20
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Read $42 from $20
    
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_lda_zpx() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // LDA $20, X ($B5 $20)
    bus.mem[0] = 0xB5;
    bus.mem[1] = 0x20;
    bus.mem[0x25] = 0x42;
    cpu.pc = 0;
    cpu.x = 5;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $B5
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $20
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Add X (Dummy read $20)
    cpu.tick(&mut bus, &mut ()); // Cycle 3: Read $42 from $25
    
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_lda_absx_no_page_cross() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // LDA $1230, X ($BD $30 $12)
    bus.mem[0] = 0xBD;
    bus.mem[1] = 0x30;
    bus.mem[2] = 0x12;
    bus.mem[0x1235] = 0x42;
    cpu.pc = 0;
    cpu.x = 5;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $BD
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $30
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Fetch $12 and Add X
    cpu.tick(&mut bus, &mut ()); // Cycle 3: Read $42 from $1235 (No page cross)
    
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_lda_absx_page_cross() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // LDA $12F0, X ($BD $F0 $12)
    bus.mem[0] = 0xBD;
    bus.mem[1] = 0xF0;
    bus.mem[2] = 0x12;
    bus.mem[0x1305] = 0x42;
    cpu.pc = 0;
    cpu.x = 0x15;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $BD
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $F0
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Fetch $12 and Add X
    cpu.tick(&mut bus, &mut ()); // Cycle 3: Dummy read (Page cross)
    cpu.tick(&mut bus, &mut ()); // Cycle 4: Read $42 from $1305
    
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_lda_indy_no_page_cross() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // LDA ($20), Y ($B1 $20)
    bus.mem[0] = 0xB1;
    bus.mem[1] = 0x20;
    bus.mem[0x20] = 0x30; // LO
    bus.mem[0x21] = 0x12; // HI -> $1230
    bus.mem[0x1235] = 0x42;
    cpu.pc = 0;
    cpu.y = 5;
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $B1
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $20
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Read LO $30
    cpu.tick(&mut bus, &mut ()); // Cycle 3: Read HI $12 and Add Y
    cpu.tick(&mut bus, &mut ()); // Cycle 4: Read $42 from $1235 (No page cross)
    
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_bne_taken_no_page_cross() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // BNE +$05 ($D0 $05)
    bus.mem[0x100] = 0xD0;
    bus.mem[0x101] = 0x05;
    cpu.pc = 0x100;
    cpu.p = 0; // Zero flag clear -> taken
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $D0
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $05 and check condition
    cpu.tick(&mut bus, &mut ()); // Cycle 2: Add offset (No page cross)
    
    assert_eq!(cpu.pc, 0x107);
    assert_eq!(cpu.cycle, 0);
}

#[test]
fn test_bne_not_taken() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    
    // BNE +$05 ($D0 $05)
    bus.mem[0x100] = 0xD0;
    bus.mem[0x101] = 0x05;
    cpu.pc = 0x100;
    cpu.p = 0x02; // Zero flag set -> not taken
    
    cpu.tick(&mut bus, &mut ()); // Cycle 0: Fetch $D0
    cpu.tick(&mut bus, &mut ()); // Cycle 1: Fetch $05 and check condition (Not taken)
    
    assert_eq!(cpu.pc, 0x102);
    assert_eq!(cpu.cycle, 0);
}
