use racc6502v4::{bus::Bus, cpu::CPU};

struct TestBus {
    mem: [u8; 65536],
}

impl Bus for TestBus {
    type Ctx = ();
    fn read(&mut self, _ctx: &mut (), addr: u16, _cycle: u32) -> u8 {
        self.mem[addr as usize]
    }
    fn write(&mut self, _ctx: &mut (), addr: u16, value: u8, _cycle: u32) {
        self.mem[addr as usize] = value;
    }

    fn get_instruction_ptr(&self, addr: u16) -> *const u8 {
        &self.mem[addr as usize]
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x30);
    assert_eq!(cycles, 2);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(bus.mem[0x1234], 0x42);
    assert_eq!(cycles, 4);
}

#[test]
fn test_reset() {
    let mut cpu = CPU::new();

    let mut bus = TestBus { mem: [0; 65536] };

    // Reset vector at $FFFC = $00, $FFFD = $C0 ($C000)
    bus.mem[0xFFFC] = 0x00;
    bus.mem[0xFFFD] = 0xC0;

    cpu.reset(&mut bus, &mut ());

    assert_eq!(cpu.pc, 0xC000);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 3);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 4);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 4);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 5);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.a, 0x42);
    assert_eq!(cycles, 5);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x107);
    assert_eq!(cycles, 3);
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

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x102);
    assert_eq!(cycles, 2);
}

#[test]
fn test_illegal_dcp_zp() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;
    cpu.a = 0x04;
    
    // DCP $10
    bus.mem[0x8000] = 0xC7;
    bus.mem[0x8001] = 0x10;
    
    bus.mem[0x0010] = 0x05;
    
    let cycles = cpu.step(&mut bus, &mut ());
    
    assert_eq!(bus.mem[0x0010], 0x04);
    assert_eq!(cpu.a, 0x04);
    assert_eq!(cpu.p & 0x02, 0x02, "Z flag not set");
    assert_eq!(cpu.p & 0x01, 0x01, "C flag not set");
    assert_eq!(cycles, 5);
}

#[test]
fn test_illegal_slo_zp() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;
    cpu.a = 0x01;
    
    // SLO $10
    bus.mem[0x8000] = 0x07;
    bus.mem[0x8001] = 0x10;
    
    bus.mem[0x0010] = 0x80;
    
    let cycles = cpu.step(&mut bus, &mut ());
    
    assert_eq!(bus.mem[0x0010], 0x00, "Memory not shifted");
    assert_eq!(cpu.a, 0x01, "A not updated correctly");
    assert_eq!(cpu.p & 0x01, 0x01, "C flag not set by ASL");
    assert_eq!(cycles, 5);
}

#[test]
fn test_illegal_arr_imm() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;
    cpu.a = 0xFF;
    cpu.p |= 0x01; // Set C flag
    
    // ARR #$F0
    bus.mem[0x8000] = 0x6B;
    bus.mem[0x8001] = 0xF0;
    
    let cycles = cpu.step(&mut bus, &mut ());
    
    assert_eq!(cpu.a, 0xF8, "ARR A calculation wrong");
    assert_eq!(cpu.p & 0x01, 0x01, "ARR C flag wrong");
    assert_eq!(cpu.p & 0x40, 0x00, "ARR V flag wrong");
    assert_eq!(cycles, 2);
}

#[test]
fn test_illegal_alr_imm() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;
    cpu.a = 0xFF;
    
    // ALR #$FE
    bus.mem[0x8000] = 0x4B;
    bus.mem[0x8001] = 0xFE;
    
    let cycles = cpu.step(&mut bus, &mut ());
    
    assert_eq!(cpu.a, 0x7F, "ALR A calculation wrong");
    assert_eq!(cpu.p & 0x01, 0x00, "ALR C flag wrong");
    assert_eq!(cycles, 2);
}

#[test]
fn test_nop_abs_0c() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;

    // NOP $0215 (0x0C 0x15 0x02) - 3 bytes, 4 cycles
    bus.mem[0x8000] = 0x0C;
    bus.mem[0x8001] = 0x15;
    bus.mem[0x8002] = 0x02;

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x8003, "NOP abs (0x0C) must advance PC by 3 bytes");
    assert_eq!(cycles, 4, "NOP abs (0x0C) must take 4 cycles");
}

#[test]
fn test_nop_absx_1c() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;
    cpu.x = 0x05;

    // NOP $1234,X (0x1C 0x34 0x12) - 3 bytes, 4 cycles (no page cross)
    bus.mem[0x8000] = 0x1C;
    bus.mem[0x8001] = 0x34;
    bus.mem[0x8002] = 0x12;

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x8003, "NOP abs,X (0x1C) must advance PC by 3 bytes");
    assert_eq!(cycles, 4, "NOP abs,X (0x1C) must take 4 cycles without page cross");

    // Page cross test
    cpu.pc = 0x8000;
    cpu.x = 0xFF;
    bus.mem[0x8001] = 0x05;
    bus.mem[0x8002] = 0x12;
    let cycles_cross = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x8003);
    assert_eq!(cycles_cross, 5, "NOP abs,X (0x1C) must take 5 cycles with page cross");
}

#[test]
fn test_nop_1byte_1a() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;

    // NOP (0x1A) - 1 byte, 2 cycles
    bus.mem[0x8000] = 0x1A;
    bus.mem[0x8001] = 0xEA;

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x8001, "NOP 1-byte (0x1A) must advance PC by 1 byte");
    assert_eq!(cycles, 2, "NOP 1-byte (0x1A) must take 2 cycles");
}

#[test]
fn test_nop_zp_04() {
    let mut cpu = CPU::new();
    let mut bus = TestBus { mem: [0; 65536] };
    cpu.pc = 0x8000;

    // NOP $10 (0x04 0x10) - 2 bytes, 3 cycles
    bus.mem[0x8000] = 0x04;
    bus.mem[0x8001] = 0x10;

    let cycles = cpu.step(&mut bus, &mut ());
    assert_eq!(cpu.pc, 0x8002, "NOP zp (0x04) must advance PC by 2 bytes");
    assert_eq!(cycles, 3, "NOP zp (0x04) must take 3 cycles");
}

#[test]
fn test_all_1byte_nops() {
    for &op in &[0x1A, 0x3A, 0x5A, 0x7A, 0xDA, 0xFA] {
        let mut cpu = CPU::new();
        let mut bus = TestBus { mem: [0; 65536] };
        cpu.pc = 0x8000;
        bus.mem[0x8000] = op;
        bus.mem[0x8001] = 0xEA;

        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8001, "Opcode 0x{:02X} must advance PC by 1", op);
        assert_eq!(cycles, 2, "Opcode 0x{:02X} must take 2 cycles", op);
    }
}

#[test]
fn test_all_2byte_imm_nops() {
    for &op in &[0x80, 0x82, 0x89, 0xC2, 0xE2] {
        let mut cpu = CPU::new();
        let mut bus = TestBus { mem: [0; 65536] };
        cpu.pc = 0x8000;
        bus.mem[0x8000] = op;
        bus.mem[0x8001] = 0x42;

        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8002, "Opcode 0x{:02X} must advance PC by 2", op);
        assert_eq!(cycles, 2, "Opcode 0x{:02X} must take 2 cycles", op);
    }
}

#[test]
fn test_all_2byte_zp_nops() {
    for &op in &[0x04, 0x44, 0x64] {
        let mut cpu = CPU::new();
        let mut bus = TestBus { mem: [0; 65536] };
        cpu.pc = 0x8000;
        bus.mem[0x8000] = op;
        bus.mem[0x8001] = 0x10;

        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8002, "Opcode 0x{:02X} must advance PC by 2", op);
        assert_eq!(cycles, 3, "Opcode 0x{:02X} must take 3 cycles", op);
    }
}

#[test]
fn test_all_2byte_zpx_nops() {
    for &op in &[0x14, 0x34, 0x54, 0x74, 0xD4, 0xF4] {
        let mut cpu = CPU::new();
        let mut bus = TestBus { mem: [0; 65536] };
        cpu.pc = 0x8000;
        cpu.x = 0x05;
        bus.mem[0x8000] = op;
        bus.mem[0x8001] = 0x10;

        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8002, "Opcode 0x{:02X} must advance PC by 2", op);
        assert_eq!(cycles, 4, "Opcode 0x{:02X} must take 4 cycles", op);
    }
}

#[test]
fn test_all_3byte_absx_nops() {
    for &op in &[0x1C, 0x3C, 0x5C, 0x7C, 0xDC, 0xFC] {
        let mut cpu = CPU::new();
        let mut bus = TestBus { mem: [0; 65536] };
        cpu.pc = 0x8000;
        cpu.x = 0x05;
        bus.mem[0x8000] = op;
        bus.mem[0x8001] = 0x34;
        bus.mem[0x8002] = 0x12;

        let cycles = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8003, "Opcode 0x{:02X} must advance PC by 3", op);
        assert_eq!(cycles, 4, "Opcode 0x{:02X} must take 4 cycles without page cross", op);

        // Page cross
        cpu.pc = 0x8000;
        cpu.x = 0xFF;
        bus.mem[0x8001] = 0x05;
        bus.mem[0x8002] = 0x12;
        let cycles_cross = cpu.step(&mut bus, &mut ());
        assert_eq!(cpu.pc, 0x8003);
        assert_eq!(cycles_cross, 5, "Opcode 0x{:02X} must take 5 cycles with page cross", op);
    }
}


