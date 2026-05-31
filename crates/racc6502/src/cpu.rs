use crate::bus::Bus;
use crate::instructions::{
    AddrMicroOp, ExecMicroOp, VECTOR_IRQ, VECTOR_NMI, VECTOR_RESET,
    nmos6502::INSTRUCTION_TABLE,
};

/// The high-level state of the 6502 execution cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleState {
    Reset,
    Interrupt,
    FetchOpcode,
    AddrMode,
    Operation,
}

#[cfg(feature = "debug")]
pub type DebugHook = fn(&CPU, *mut core::ffi::c_void);

/// The cycle-accurate MOS 6502 CPU core.
pub struct CPU {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub pc: u16,
    pub s: u8,
    pub p: u8,
    pub ir: u8,

    pub active_addr: &'static [AddrMicroOp],
    pub active_exec: &'static [ExecMicroOp],
    pub step_index: usize,
    pub state: CycleState,

    pub effective_addr: u16,
    pub data_bus: u8,
    pub temp_data: u8,
    pub page_crossed: bool,
    pub pending_vector: u16,

    #[cfg(feature = "debug")]
    pub debug_hook: Option<DebugHook>,
    #[cfg(feature = "debug")]
    pub debug_user_data: *mut core::ffi::c_void,
    #[cfg(feature = "debug")]
    pub debug_interrupt_level: u8,
}

impl CPU {
    pub fn new() -> Self {
        let mut cpu = CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0xFD,
            p: 0x24,
            ir: 0,
            active_addr: &[],
            active_exec: &[],
            step_index: 0,
            state: CycleState::Reset,
            effective_addr: 0,
            data_bus: 0,
            temp_data: 0,
            page_crossed: false,
            pending_vector: 0,
            #[cfg(feature = "debug")]
            debug_hook: None,
            #[cfg(feature = "debug")]
            debug_user_data: core::ptr::null_mut(),
            #[cfg(feature = "debug")]
            debug_interrupt_level: 0,
        };
        cpu.reset();
        cpu
    }

    pub fn reset(&mut self) {
        self.state = CycleState::Reset;
        self.step_index = 0;
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    pub fn tick<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if bus.so_pending() {
            self.p |= 0x40;
        }

        match self.state {
            CycleState::Reset => self.tick_reset(bus, ctx),
            CycleState::Interrupt => self.tick_interrupt(bus, ctx),
            CycleState::FetchOpcode => self.tick_fetch(bus, ctx),
            CycleState::AddrMode => self.tick_addr(bus, ctx),
            CycleState::Operation => self.tick_operation(bus, ctx),
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn tick_reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        match self.step_index {
            0..=2 => { bus.read(self.pc, ctx); self.step_index += 1; }
            3..=5 => { bus.read(0x0100 | (self.s as u16), ctx); self.step_index += 1; }
            6 => { self.pc = bus.read(VECTOR_RESET, ctx) as u16; self.step_index += 1; }
            7 => {
                let hi = bus.read(VECTOR_RESET + 1, ctx) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
            }
            _ => unreachable!(),
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn tick_interrupt<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        match self.step_index {
            0 => { bus.read(self.pc, ctx); self.step_index += 1; }
            1 => { bus.read(self.pc, ctx); self.step_index += 1; }
            2 => { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); self.step_index += 1; }
            3 => { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); self.step_index += 1; }
            4 => { bus.write(0x0100 | (self.s as u16), self.p | 0x20, ctx); self.s = self.s.wrapping_sub(1); self.p |= 0x04; self.step_index += 1; }
            5 => { self.pc = bus.read(self.pending_vector, ctx) as u16; self.step_index += 1; }
            6 => {
                let hi = bus.read(self.pending_vector + 1, ctx) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
            }
            _ => unreachable!(),
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn tick_fetch<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if bus.nmi_pending() {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_NMI;
            self.step_index = 0;
            self.tick_interrupt(bus, ctx);
            return;
        }
        if bus.irq_pending() && (self.p & 0x04) == 0 {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_IRQ;
            self.step_index = 0;
            self.tick_interrupt(bus, ctx);
            return;
        }

        self.ir = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);

        let instr = unsafe { INSTRUCTION_TABLE.get_unchecked(self.ir as usize) };
        self.active_addr = instr.addr;
        self.active_exec = instr.exec;

        #[cfg(feature = "debug")]
        if let Some(hook) = self.debug_hook {
            hook(self, self.debug_user_data);
        }

        self.state = CycleState::AddrMode;
        self.step_index = 0;
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn tick_addr<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if self.step_index < self.active_addr.len() {
            let uop = unsafe { *self.active_addr.get_unchecked(self.step_index) };
            if (uop == AddrMicroOp::OptionalFixupX || uop == AddrMicroOp::OptionalFixupY || uop == AddrMicroOp::OptionalFixupIndY) && !self.page_crossed {
                self.step_index += 1;
                if self.step_index < self.active_addr.len() {
                    let next_uop = unsafe { *self.active_addr.get_unchecked(self.step_index) };
                    self.execute_addr_uop(next_uop, bus, ctx);
                    self.step_index += 1;
                } else {
                    self.state = CycleState::Operation; self.step_index = 0; self.tick_operation(bus, ctx); return;
                }
            } else {
                self.execute_addr_uop(uop, bus, ctx);
                self.step_index += 1;
            }
        } else {
            self.state = CycleState::Operation; self.step_index = 0; self.tick_operation(bus, ctx);
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn tick_operation<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if self.step_index < self.active_exec.len() {
            let uop = unsafe { *self.active_exec.get_unchecked(self.step_index) };
            self.execute_exec_uop(uop, bus, ctx);
            self.step_index += 1;
        } else {
            self.state = CycleState::FetchOpcode; self.step_index = 0; self.tick_fetch(bus, ctx);
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn update_zn(&mut self, val: u8) {
        self.p &= 0x7D;
        if val == 0 { self.p |= 0x02; }
        self.p |= val & 0x80;
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn execute_addr_uop<B: Bus>(&mut self, uop: AddrMicroOp, bus: &mut B, ctx: &mut B::Ctx) {
        match uop {
            AddrMicroOp::FetchAddrLo => { let lo = bus.read(self.pc, ctx); self.effective_addr = lo as u16; self.pc = self.pc.wrapping_add(1); }
            AddrMicroOp::FetchAddrHi => { let hi = bus.read(self.pc, ctx); self.effective_addr |= (hi as u16) << 8; self.pc = self.pc.wrapping_add(1); }
            AddrMicroOp::FetchAddrHiAndJump => { let hi = bus.read(self.pc, ctx) as u16; self.pc = (hi << 8) | (self.effective_addr & 0x00FF); }
            AddrMicroOp::AddXToZP => { let lo = self.effective_addr as u8; bus.read(self.effective_addr, ctx); self.effective_addr = lo.wrapping_add(self.x) as u16; }
            AddrMicroOp::AddYToZP => { let lo = self.effective_addr as u8; bus.read(self.effective_addr, ctx); self.effective_addr = lo.wrapping_add(self.y) as u16; }
            AddrMicroOp::AddXToAbs => { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.x); self.effective_addr = (hi << 8) | (lo as u16); self.page_crossed = carry; }
            AddrMicroOp::AddYToAbs => { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.y); self.effective_addr = (hi << 8) | (lo as u16); self.page_crossed = carry; }
            AddrMicroOp::FetchIndXLo => { self.data_bus = bus.read(self.effective_addr, ctx); self.effective_addr = (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF); }
            AddrMicroOp::FetchIndXHi => { let hi = bus.read(self.effective_addr, ctx) as u16; self.effective_addr = (hi << 8) | (self.data_bus as u16); }
            AddrMicroOp::FetchIndYLo => { self.data_bus = bus.read(self.effective_addr, ctx); self.effective_addr = (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF); }
            AddrMicroOp::FetchIndYHi => { let hi = bus.read(self.effective_addr, ctx) as u16; let (new_al, carry) = (self.data_bus).overflowing_add(self.y); self.effective_addr = (hi << 8) | (new_al as u16); self.page_crossed = carry; }
            AddrMicroOp::OptionalFixupX | AddrMicroOp::OptionalFixupY | AddrMicroOp::OptionalFixupIndY |
            AddrMicroOp::MandatoryFixupX | AddrMicroOp::MandatoryFixupY | AddrMicroOp::MandatoryFixupIndY => {
                bus.read(self.effective_addr, ctx);
                if self.page_crossed { self.effective_addr = self.effective_addr.wrapping_add(0x100); }
            }
            AddrMicroOp::DummyReadPC => { bus.read(self.pc, ctx); }
            AddrMicroOp::DummyReadEff => { bus.read(self.effective_addr, ctx); }
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn execute_exec_uop<B: Bus>(&mut self, uop: ExecMicroOp, bus: &mut B, ctx: &mut B::Ctx) {
        match uop {
            ExecMicroOp::ReadEffAndLDA => { self.a = bus.read(self.effective_addr, ctx); self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndLDX => { self.x = bus.read(self.effective_addr, ctx); self.update_zn(self.x); }
            ExecMicroOp::ReadEffAndLDY => { self.y = bus.read(self.effective_addr, ctx); self.update_zn(self.y); }
            ExecMicroOp::ReadEffAndADC => { let val = bus.read(self.effective_addr, ctx); self.do_adc(val); }
            ExecMicroOp::ReadEffAndSBC => { let val = bus.read(self.effective_addr, ctx); self.do_sbc(val); }
            ExecMicroOp::ReadEffAndAND => { let val = bus.read(self.effective_addr, ctx); self.a &= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndORA => { let val = bus.read(self.effective_addr, ctx); self.a |= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndEOR => { let val = bus.read(self.effective_addr, ctx); self.a ^= val; self.update_zn(self.a); }
            ExecMicroOp::ReadEffAndBIT => { let val = bus.read(self.effective_addr, ctx); self.do_bit(val); }
            ExecMicroOp::ReadEffAndCMP => { let val = bus.read(self.effective_addr, ctx); self.do_cmp(self.a, val); }
            ExecMicroOp::ReadEffAndCPX => { let val = bus.read(self.effective_addr, ctx); self.do_cmp(self.x, val); }
            ExecMicroOp::ReadEffAndCPY => { let val = bus.read(self.effective_addr, ctx); self.do_cmp(self.y, val); }
            ExecMicroOp::FetchImmAndLDA => { self.a = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndLDX => { self.x = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.update_zn(self.x); }
            ExecMicroOp::FetchImmAndLDY => { self.y = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.update_zn(self.y); }
            ExecMicroOp::FetchImmAndADC => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.do_adc(val); }
            ExecMicroOp::FetchImmAndSBC => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.do_sbc(val); }
            ExecMicroOp::FetchImmAndAND => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.a &= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndORA => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.a |= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndEOR => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.a ^= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndCMP => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.do_cmp(self.a, val); }
            ExecMicroOp::FetchImmAndCPX => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.do_cmp(self.x, val); }
            ExecMicroOp::FetchImmAndCPY => { let val = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.do_cmp(self.y, val); }
            ExecMicroOp::WriteEffA => { bus.write(self.effective_addr, self.a, ctx); }
            ExecMicroOp::WriteEffX => { bus.write(self.effective_addr, self.x, ctx); }
            ExecMicroOp::WriteEffY => { bus.write(self.effective_addr, self.y, ctx); }
            ExecMicroOp::ReadEffToData => { self.data_bus = bus.read(self.effective_addr, ctx); }
            ExecMicroOp::WriteDataBack => { bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::ASLWrite => { let c = (self.data_bus & 0x80) != 0; self.data_bus <<= 1; self.set_flag(0, c); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::LSRWrite => { let c = (self.data_bus & 0x01) != 0; self.data_bus >>= 1; self.set_flag(0, c); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::ROLWrite => { let c = (self.data_bus & 0x80) != 0; self.data_bus = (self.data_bus << 1) | (self.get_flag(0) as u8); self.set_flag(0, c); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::RORWrite => { let c = (self.data_bus & 0x01) != 0; self.data_bus = (self.data_bus >> 1) | ((self.get_flag(0) as u8) << 7); self.set_flag(0, c); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::INCWrite => { self.data_bus = self.data_bus.wrapping_add(1); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::DECWrite => { self.data_bus = self.data_bus.wrapping_sub(1); self.update_zn(self.data_bus); bus.write(self.effective_addr, self.data_bus, ctx); }
            ExecMicroOp::AccASL => { let c = (self.a & 0x80) != 0; self.a <<= 1; self.set_flag(0, c); self.update_zn(self.a); }
            ExecMicroOp::AccLSR => { let c = (self.a & 0x01) != 0; self.a >>= 1; self.set_flag(0, c); self.update_zn(self.a); }
            ExecMicroOp::AccROL => { let c = (self.a & 0x80) != 0; self.a = (self.a << 1) | (self.get_flag(0) as u8); self.set_flag(0, c); self.update_zn(self.a); }
            ExecMicroOp::AccROR => { let c = (self.a & 0x01) != 0; self.a = (self.a >> 1) | ((self.get_flag(0) as u8) << 7); self.set_flag(0, c); self.update_zn(self.a); }
            ExecMicroOp::PushA => { bus.write(0x0100 | (self.s as u16), self.a, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushP => { bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushPCH => { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushPCL => { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PullA => { self.s = self.s.wrapping_add(1); self.a = bus.read(0x0100 | (self.s as u16), ctx); self.update_zn(self.a); }
            ExecMicroOp::PullP => { self.s = self.s.wrapping_add(1); self.p = (bus.read(0x0100 | (self.s as u16), ctx) & 0xCF) | (self.p & 0x30); }
            ExecMicroOp::PullPCH => { self.s = self.s.wrapping_add(1); let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.pc = (hi << 8) | (self.pc & 0x00FF); }
            ExecMicroOp::PullPCL => { self.s = self.s.wrapping_add(1); let lo = bus.read(0x0100 | (self.s as u16), ctx); self.pc = (self.pc & 0xFF00) | (lo as u16); }
            ExecMicroOp::DummyReadStack => { bus.read(0x0100 | (self.s as u16), ctx); }
            ExecMicroOp::TAX => { self.x = self.a; self.update_zn(self.x); bus.read(self.pc, ctx); }
            ExecMicroOp::TXA => { self.a = self.x; self.update_zn(self.a); bus.read(self.pc, ctx); }
            ExecMicroOp::TAY => { self.y = self.a; self.update_zn(self.y); bus.read(self.pc, ctx); }
            ExecMicroOp::TYA => { self.a = self.y; self.update_zn(self.a); bus.read(self.pc, ctx); }
            ExecMicroOp::TSX => { self.x = self.s; self.update_zn(self.x); bus.read(self.pc, ctx); }
            ExecMicroOp::TXS => { self.s = self.x; bus.read(self.pc, ctx); }
            ExecMicroOp::CLC => { self.p &= !0x01; bus.read(self.pc, ctx); }
            ExecMicroOp::SEC => { self.p |= 0x01; bus.read(self.pc, ctx); }
            ExecMicroOp::CLI => { self.p &= !0x04; bus.read(self.pc, ctx); }
            ExecMicroOp::SEI => { self.p |= 0x04; bus.read(self.pc, ctx); }
            ExecMicroOp::CLV => { self.p &= !0x40; bus.read(self.pc, ctx); }
            ExecMicroOp::CLD => { self.p &= !0x08; bus.read(self.pc, ctx); }
            ExecMicroOp::SED => { self.p |= 0x08; bus.read(self.pc, ctx); }
            ExecMicroOp::INX => { self.x = self.x.wrapping_add(1); self.update_zn(self.x); bus.read(self.pc, ctx); }
            ExecMicroOp::DEX => { self.x = self.x.wrapping_sub(1); self.update_zn(self.x); bus.read(self.pc, ctx); }
            ExecMicroOp::INY => { self.y = self.y.wrapping_add(1); self.update_zn(self.y); bus.read(self.pc, ctx); }
            ExecMicroOp::DEY => { self.y = self.y.wrapping_sub(1); self.update_zn(self.y); bus.read(self.pc, ctx); }
            ExecMicroOp::JumpIndCycle4 => { self.data_bus = bus.read(self.effective_addr, ctx); }
            ExecMicroOp::JumpIndCycle5 => { let hi = bus.read((self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF), ctx) as u16; self.pc = (hi << 8) | (self.data_bus as u16); }
            ExecMicroOp::BranchCycle2 => {
                let offset = bus.read(self.pc, ctx) as i8; self.pc = self.pc.wrapping_add(1);
                let condition = match self.ir >> 6 { 0 => (self.p & 0x80) != 0, 1 => (self.p & 0x40) != 0, 2 => (self.p & 0x01) != 0, 3 => (self.p & 0x02) != 0, _ => unreachable!() };
                if condition ^ ((self.ir & 0x20) == 0) {
                    let old_pc = self.pc; self.pc = self.pc.wrapping_add(offset as u16);
                    if (old_pc & 0xFF00) != (self.pc & 0xFF00) { self.page_crossed = true; }
                } else { self.state = CycleState::FetchOpcode; self.step_index = 0; }
            }
            ExecMicroOp::BranchCycle3 => { bus.read(self.pc, ctx); if !self.page_crossed { self.state = CycleState::FetchOpcode; self.step_index = 0; } }
            ExecMicroOp::BranchCycle4 => { bus.read(self.pc, ctx); self.state = CycleState::FetchOpcode; self.step_index = 0; }
            ExecMicroOp::JsrCycle2 => { self.data_bus = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::JsrCycle3 => { bus.read(0x0100 | (self.s as u16), ctx); }
            ExecMicroOp::JsrCycle4 => { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::JsrCycle5 => { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::JsrCycle6 => { let hi = bus.read(self.pc, ctx) as u16; self.pc = (hi << 8) | (self.data_bus as u16); }
            ExecMicroOp::RtsCycle2 => { bus.read(self.pc, ctx); }
            ExecMicroOp::RtsCycle3 => { bus.read(0x0100 | (self.s as u16), ctx); }
            ExecMicroOp::RtsCycle4 => { self.s = self.s.wrapping_add(1); let lo = bus.read(0x0100 | (self.s as u16), ctx); self.pc = (self.pc & 0xFF00) | (lo as u16); }
            ExecMicroOp::RtsCycle5 => { self.s = self.s.wrapping_add(1); let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.pc = (hi << 8) | (self.pc & 0x00FF); }
            ExecMicroOp::RtsCycle6 => { bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::RtiCycle2 => { bus.read(self.pc, ctx); }
            ExecMicroOp::RtiCycle3 => { bus.read(0x0100 | (self.s as u16), ctx); }
            ExecMicroOp::RtiCycle4 => { self.s = self.s.wrapping_add(1); self.p = (bus.read(0x0100 | (self.s as u16), ctx) & 0xCF) | (self.p & 0x30); }
            ExecMicroOp::RtiCycle5 => { self.s = self.s.wrapping_add(1); let lo = bus.read(0x0100 | (self.s as u16), ctx); self.pc = (self.pc & 0xFF00) | (lo as u16); }
            ExecMicroOp::RtiCycle6 => { self.s = self.s.wrapping_add(1); let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.pc = (hi << 8) | (self.pc & 0x00FF); }
            ExecMicroOp::BrkCycle2 => { bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::BrkCycle3 => { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::BrkCycle4 => { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::BrkCycle5 => { bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx); self.s = self.s.wrapping_sub(1); self.p |= 0x04; }
            ExecMicroOp::BrkCycle6 => { self.data_bus = bus.read(VECTOR_IRQ, ctx); }
            ExecMicroOp::BrkCycle7 => { let hi = bus.read(VECTOR_IRQ + 1, ctx) as u16; self.pc = (hi << 8) | (self.data_bus as u16); }
            ExecMicroOp::DummyReadPC => { bus.read(self.pc, ctx); }
            ExecMicroOp::DummyReadEff => { bus.read(self.effective_addr, ctx); }
            ExecMicroOp::Unimplemented => { bus.read(self.pc, ctx); }
        }
    }

    #[inline(always)]
    pub fn get_flag(&self, bit: u8) -> bool { (self.p & (1 << bit)) != 0 }
    #[inline(always)]
    pub fn set_flag(&mut self, bit: u8, val: bool) { if val { self.p |= 1 << bit; } else { self.p &= !(1 << bit); } }

    fn do_adc(&mut self, val: u8) {
        if (self.p & 0x08) != 0 {
            let mut low = (self.a & 0x0F) as i32 + (val & 0x0F) as i32 + self.get_flag(0) as i32;
            let mut high = (self.a >> 4) as i32 + (val >> 4) as i32 + if low > 9 { 1 } else { 0 };
            if low > 9 { low += 6; }
            
            let a = self.a as u16; let v = val as u16; let c = self.get_flag(0) as u16;
            let res = a + v + c;
            self.set_flag(6, ((a ^ res) & (v ^ res) & 0x80) != 0);
            
            if high > 9 { high += 6; }
            self.set_flag(0, high > 15);
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
            self.update_zn(self.a);
        } else {
            let a = self.a as u16; let v = val as u16; let c = self.get_flag(0) as u16;
            let res = a + v + c; self.set_flag(0, res > 0xFF); self.set_flag(6, ((a ^ res) & (v ^ res) & 0x80) != 0);
            self.a = res as u8; self.update_zn(self.a);
        }
    }
    fn do_sbc(&mut self, val: u8) {
        if (self.p & 0x08) != 0 {
            let mut low = (self.a & 0x0F) as i32 - (val & 0x0F) as i32 - (1 - self.get_flag(0) as i32);
            let mut high = (self.a >> 4) as i32 - (val >> 4) as i32 - if low < 0 { 1 } else { 0 };
            if low < 0 { low -= 6; }
            
            let a = self.a as u16; let v = (!val) as u16; let c = self.get_flag(0) as u16;
            let res = a + v + c;
            self.set_flag(0, res > 0xFF);
            self.set_flag(6, ((a ^ res) & (v ^ res) & 0x80) != 0);
            
            if high < 0 { high -= 6; }
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
            self.update_zn(self.a);
        } else {
            self.do_adc(!val);
        }
    }
    fn do_bit(&mut self, val: u8) { self.set_flag(1, (self.a & val) == 0); self.p = (self.p & 0x3F) | (val & 0xC0); }
    fn do_cmp(&mut self, reg: u8, val: u8) { let (res, c) = reg.overflowing_sub(val); self.set_flag(0, !c); self.update_zn(res); }
}
