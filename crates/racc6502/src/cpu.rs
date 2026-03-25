use crate::bus::Bus;
use crate::instructions::{AddrMicroOp, ExecMicroOp, VECTOR_RESET, VECTOR_NMI, VECTOR_IRQ};
use crate::instructions::nmos6502::INSTRUCTION_TABLE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleState {
    Reset,
    Interrupt,
    FetchOpcode,
    AddrMode,
    Operation,
}

pub struct CPU<B: Bus> {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub s: u8,
    pub p: u8, // Processor Status

    pub ir: u8, // Instruction Register
    pub step_index: usize,
    pub state: CycleState,

    pub effective_addr: u16,
    pub data_bus: u8,

    pub pending_vector: u16,
    pub bus: B,
}

impl<B: Bus> CPU<B> {
    pub fn new(bus: B) -> Self {
        let mut cpu = CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0xFD,
            p: 0x34,
            ir: 0,
            step_index: 0,
            state: CycleState::Reset,
            effective_addr: 0,
            data_bus: 0,
            pending_vector: 0,
            bus,
        };
        cpu.reset();
        cpu
    }

    pub fn reset(&mut self) {
        self.state = CycleState::Reset;
        self.step_index = 0;
    }

    pub fn tick(&mut self) {
        // Handle SO (Set Overflow) pin transition
        if self.bus.so_pending() {
            self.p |= 0x40; // Set Overflow flag
        }

        match self.state {
            CycleState::Reset => self.tick_reset(),
            CycleState::Interrupt => self.tick_interrupt(),
            CycleState::FetchOpcode => self.tick_fetch(),
            CycleState::AddrMode => self.tick_addr(),
            CycleState::Operation => self.tick_operation(),
        }
    }

    fn tick_reset(&mut self) {
        match self.step_index {
            0..=2 => {
                self.bus.read(self.pc);
                self.step_index += 1;
            }
            3..=5 => {
                self.bus.read(0x0100 | (self.s as u16));
                self.step_index += 1;
            }
            6 => {
                self.pc = self.bus.read(VECTOR_RESET) as u16;
                self.step_index += 1;
            }
            7 => {
                let hi = self.bus.read(VECTOR_RESET + 1) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
            }
            _ => unreachable!(),
        }
    }

    fn tick_interrupt(&mut self) {
        match self.step_index {
            0 => {
                self.bus.read(self.pc); // Dummy opcode fetch
                self.step_index += 1;
            }
            1 => {
                self.bus.read(self.pc); // Dummy read
                self.step_index += 1;
            }
            2 => {
                self.bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8);
                self.s = self.s.wrapping_sub(1);
                self.step_index += 1;
            }
            3 => {
                self.bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8);
                self.s = self.s.wrapping_sub(1);
                self.step_index += 1;
            }
            4 => {
                self.bus.write(0x0100 | (self.s as u16), self.p | 0x20); // B=0
                self.s = self.s.wrapping_sub(1);
                self.p |= 0x04; // Set I flag
                self.step_index += 1;
            }
            5 => {
                self.pc = self.bus.read(self.pending_vector) as u16;
                self.step_index += 1;
            }
            6 => {
                let hi = self.bus.read(self.pending_vector + 1) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
            }
            _ => unreachable!(),
        }
    }

    fn tick_fetch(&mut self) {
        if self.bus.nmi_pending() {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_NMI;
            self.step_index = 0;
            self.tick_interrupt();
            return;
        }
        if self.bus.irq_pending() && (self.p & 0x04 == 0) {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_IRQ;
            self.step_index = 0;
            self.tick_interrupt();
            return;
        }

        self.ir = self.bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        self.state = CycleState::AddrMode;
        self.step_index = 0;
    }

    fn tick_addr(&mut self) {
        let instruction = &INSTRUCTION_TABLE[self.ir as usize];
        let current_slice = instruction.addr;

        if current_slice.is_empty() {
            self.state = CycleState::Operation;
            self.step_index = 0;
            self.tick_operation();
            return;
        }

        if self.step_index < current_slice.len() {
            let uop = current_slice[self.step_index];
            self.execute_addr_uop(uop);
            self.step_index += 1;
        }
        
        if self.step_index >= current_slice.len() {
            self.state = CycleState::Operation;
            self.step_index = 0;
        }
    }

    fn tick_operation(&mut self) {
        let instruction = &INSTRUCTION_TABLE[self.ir as usize];
        let current_slice = instruction.exec;

        if current_slice.is_empty() {
            self.bus.read(self.pc); // Mandatory bus access
            self.state = CycleState::FetchOpcode;
            self.step_index = 0;
            return;
        }

        if self.step_index < current_slice.len() {
            let uop = current_slice[self.step_index];
            self.execute_exec_uop(uop);
            self.step_index += 1;
        }

        if self.step_index >= current_slice.len() {
            self.state = CycleState::FetchOpcode;
            self.step_index = 0;
        }
    }

    fn execute_addr_uop(&mut self, uop: AddrMicroOp) {
        match uop {
            AddrMicroOp::FetchAddrLo => {
                self.effective_addr = self.bus.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::FetchAddrHi => {
                let hi = self.bus.read(self.pc) as u16;
                self.effective_addr |= hi << 8;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::DummyReadPC => { self.bus.read(self.pc); }
            AddrMicroOp::DummyReadEff => { self.bus.read(self.effective_addr); }
            AddrMicroOp::AddXToZP => {
                let lo = self.effective_addr as u8;
                self.bus.read(self.effective_addr);
                self.effective_addr = lo.wrapping_add(self.x) as u16;
            }
            AddrMicroOp::AddYToZP => {
                let lo = self.effective_addr as u8;
                self.bus.read(self.effective_addr);
                self.effective_addr = lo.wrapping_add(self.y) as u16;
            }
            AddrMicroOp::AddXToAbs => {
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.x);
                let base_high = self.effective_addr & 0xFF00;
                self.effective_addr = base_high | (lo as u16);
                self.bus.read(self.effective_addr);
                if carry { self.effective_addr = self.effective_addr.wrapping_add(0x100); }
            }
            AddrMicroOp::AddYToAbs => {
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.y);
                let base_high = self.effective_addr & 0xFF00;
                self.effective_addr = base_high | (lo as u16);
                self.bus.read(self.effective_addr);
                if carry { self.effective_addr = self.effective_addr.wrapping_add(0x100); }
            }
        }
    }

    fn execute_exec_uop(&mut self, uop: ExecMicroOp) {
        match uop {
            ExecMicroOp::ReadEff => { self.data_bus = self.bus.read(self.effective_addr); }
            ExecMicroOp::ReadEffAndLDA => { self.a = self.bus.read(self.effective_addr); self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndLDX => { self.x = self.bus.read(self.effective_addr); self.update_zn(self.x); }
            ExecMicroOp::ReadEffAndLDY => { self.y = self.bus.read(self.effective_addr); self.update_zn(self.y); }
            ExecMicroOp::ReadEffAndADC => { let val = self.bus.read(self.effective_addr); self.do_adc(val); }
            ExecMicroOp::ReadEffAndSBC => { let val = self.bus.read(self.effective_addr); self.do_sbc(val); }
            ExecMicroOp::ReadEffAndAND => { let val = self.bus.read(self.effective_addr); self.a &= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndORA => { let val = self.bus.read(self.effective_addr); self.a |= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndEOR => { let val = self.bus.read(self.effective_addr); self.a ^= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndBIT => { let val = self.bus.read(self.effective_addr); self.do_bit(val); }
            ExecMicroOp::ReadEffAndCMP => { let val = self.bus.read(self.effective_addr); self.do_compare(self.a, val); }
            ExecMicroOp::ReadEffAndCPX => { let val = self.bus.read(self.effective_addr); self.do_compare(self.x, val); }
            ExecMicroOp::ReadEffAndCPY => { let val = self.bus.read(self.effective_addr); self.do_compare(self.y, val); }
            ExecMicroOp::FetchImmAndLDA => { self.a = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndLDX => { self.x = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.update_zn(self.x); }
            ExecMicroOp::FetchImmAndLDY => { self.y = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.update_zn(self.y); }
            ExecMicroOp::FetchImmAndADC => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.do_adc(val); }
            ExecMicroOp::FetchImmAndSBC => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.do_sbc(val); }
            ExecMicroOp::FetchImmAndAND => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.a &= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndORA => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.a |= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndEOR => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.a ^= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndCMP => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.do_compare(self.a, val); }
            ExecMicroOp::FetchImmAndCPX => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.do_compare(self.x, val); }
            ExecMicroOp::FetchImmAndCPY => { let val = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); self.do_compare(self.y, val); }
            ExecMicroOp::WriteEffA => { self.bus.write(self.effective_addr, self.a); }
            ExecMicroOp::WriteEffX => { self.bus.write(self.effective_addr, self.x); }
            ExecMicroOp::WriteEffY => { self.bus.write(self.effective_addr, self.y); }
            ExecMicroOp::TAX => { self.bus.read(self.pc); self.x = self.a; self.update_zn(self.x); }
            ExecMicroOp::TXA => { self.bus.read(self.pc); self.a = self.x; self.update_zn(self.a); }
            ExecMicroOp::TAY => { self.bus.read(self.pc); self.y = self.a; self.update_zn(self.y); }
            ExecMicroOp::TYA => { self.bus.read(self.pc); self.a = self.y; self.update_zn(self.a); }
            ExecMicroOp::TSX => { self.bus.read(self.pc); self.x = self.s; self.update_zn(self.x); }
            ExecMicroOp::TXS => { self.bus.read(self.pc); self.s = self.x; }
            ExecMicroOp::INX => { self.bus.read(self.pc); self.x = self.x.wrapping_add(1); self.update_zn(self.x); }
            ExecMicroOp::DEX => { self.bus.read(self.pc); self.x = self.x.wrapping_sub(1); self.update_zn(self.x); }
            ExecMicroOp::INY => { self.bus.read(self.pc); self.y = self.y.wrapping_add(1); self.update_zn(self.y); }
            ExecMicroOp::DEY => { self.bus.read(self.pc); self.y = self.y.wrapping_sub(1); self.update_zn(self.y); }
            ExecMicroOp::CLC => { self.bus.read(self.pc); self.set_flag(0, false); }
            ExecMicroOp::SEC => { self.bus.read(self.pc); self.set_flag(0, true); }
            ExecMicroOp::CLI => { self.bus.read(self.pc); self.set_flag(2, false); }
            ExecMicroOp::SEI => { self.bus.read(self.pc); self.set_flag(2, true); }
            ExecMicroOp::CLV => { self.bus.read(self.pc); self.set_flag(6, false); }
            ExecMicroOp::CLD => { self.bus.read(self.pc); self.set_flag(3, false); }
            ExecMicroOp::SED => { self.bus.read(self.pc); self.set_flag(3, true); }
            ExecMicroOp::PushA => { self.bus.write(0x0100 | (self.s as u16), self.a); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushP => { self.bus.write(0x0100 | (self.s as u16), self.p | 0x30); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PullA => { self.s = self.s.wrapping_add(1); self.a = self.bus.read(0x0100 | (self.s as u16)); self.update_zn(self.a); }
            ExecMicroOp::PullP => { self.s = self.s.wrapping_add(1); self.p = (self.bus.read(0x0100 | (self.s as u16)) & 0xCF) | (self.p & 0x30); }
            ExecMicroOp::DummyReadPC => { self.bus.read(self.pc); }
            ExecMicroOp::DummyReadEff => { self.bus.read(self.effective_addr); }
            _ => { self.bus.read(self.pc); }
        }
    }

    fn update_zn(&mut self, val: u8) { self.set_flag(1, val == 0); self.set_flag(7, (val & 0x80) != 0); }
    fn set_flag(&mut self, bit: u8, val: bool) { if val { self.p |= 1 << bit; } else { self.p &= !(1 << bit); } }
    fn get_flag(&self, bit: u8) -> bool { (self.p & (1 << bit)) != 0 }
    fn do_compare(&mut self, reg: u8, val: u8) { let (res, borrow) = reg.overflowing_sub(val); self.set_flag(0, !borrow); self.update_zn(res); }
    fn do_adc(&mut self, val: u8) {
        let carry = if self.get_flag(0) { 1 } else { 0 };
        let a = self.a as u16;
        let v = val as u16;
        let result = a + v + carry;
        self.set_flag(0, result > 0xFF);
        let result_u8 = (result & 0xFF) as u8;
        let overflow = (!(a as u8 ^ val) & (a as u8 ^ result_u8) & 0x80) != 0;
        self.set_flag(6, overflow);
        self.a = result_u8;
        self.update_zn(self.a);
    }
    fn do_sbc(&mut self, val: u8) { self.do_adc(!val); }
    fn do_bit(&mut self, val: u8) { self.set_flag(1, (self.a & val) == 0); self.set_flag(7, (val & 0x80) != 0); self.set_flag(6, (val & 0x40) != 0); }
}
