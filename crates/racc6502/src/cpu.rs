use crate::bus::Bus;
use crate::instructions::nmos6502::INSTRUCTION_TABLE;
use crate::instructions::{AddrMicroOp, ExecMicroOp, VECTOR_IRQ, VECTOR_NMI, VECTOR_RESET};

/// Represents the current stage of the CPU's execution cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleState {
    /// Performing the 7-cycle reset sequence.
    Reset,
    /// Performing the 7-cycle interrupt sequence (IRQ/NMI).
    Interrupt,
    /// Fetching the next opcode from the bus.
    FetchOpcode,
    /// Executing the micro-ops associated with the addressing mode.
    AddrMode,
    /// Executing the micro-ops associated with the instruction's operation.
    Operation,
}

/// A hook called at the end of every instruction (the "Sync" point).
///
/// Only available when the `debug` feature is enabled.
#[cfg(feature = "debug")]
pub type DebugHook = fn(&CPU, *mut core::ffi::c_void);

/// The cycle-accurate MOS 6502 CPU core.
///
/// The CPU holds only register and execution state.  The memory bus and any
/// external context are passed into [`CPU::tick`] on every call, so they can be
/// owned by the caller rather than embedded here.
pub struct CPU {
    /// Accumulator register.
    pub a: u8,
    /// X index register.
    pub x: u8,
    /// Y index register.
    pub y: u8,
    /// Program Counter.
    pub pc: u16,
    /// Stack Pointer (offset from $0100).
    pub s: u8,
    /// Processor Status register (NV-BDIZC).
    pub p: u8,

    /// Instruction Register (current opcode).
    pub ir: u8,
    /// The index of the current micro-operation within the active slice.
    pub step_index: usize,
    /// The current high-level state of the execution cycle.
    pub state: CycleState,

    /// Intermediate address buffer used during address calculation.
    pub effective_addr: u16,
    /// Intermediate data buffer used during multi-cycle instructions.
    pub data_bus: u8,
    /// Intermediate data buffer used for Read-Modify-Write instructions.
    pub temp_data: u8,
    /// Internal flag indicating that a page boundary was crossed during indexing.
    pub page_crossed: bool,

    /// The hardware vector to jump to after the current interrupt sequence.
    pub pending_vector: u16,

    /// An optional hook called at the end of every instruction.
    #[cfg(feature = "debug")]
    pub debug_hook: Option<DebugHook>,
    /// Optional user data pointer passed to the debug hook.
    #[cfg(feature = "debug")]
    pub debug_user_data: *mut core::ffi::c_void,
    /// The current interrupt nesting level (0 = main thread).
    #[cfg(feature = "debug")]
    pub debug_interrupt_level: u8,
}

impl CPU {
    /// Creates a new CPU instance in the `Reset` state.
    pub fn new() -> Self {
        let mut cpu = CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0xFD,
            p: 0x24,
            ir: 0,
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

    /// Triggers a hard reset of the CPU.
    pub fn reset(&mut self) {
        self.state = CycleState::Reset;
        self.step_index = 0;
    }

    /// Executes exactly one clock cycle of the CPU.
    ///
    /// `bus` provides memory access; `ctx` is threaded through every bus read/write
    /// so that the caller can own external state (e.g. a hardware interface) without
    /// embedding it inside the bus.
    pub fn tick<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if bus.so_pending() {
            self.set_flag(6, true);
        }

        match self.state {
            CycleState::Reset => self.tick_reset(bus, ctx),
            CycleState::Interrupt => self.tick_interrupt(bus, ctx),
            CycleState::FetchOpcode => self.tick_fetch(bus, ctx),
            CycleState::AddrMode => self.tick_addr(bus, ctx),
            CycleState::Operation => self.tick_operation(bus, ctx),
        }
    }

    fn tick_reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        match self.step_index {
            0..=2 => {
                bus.read(self.pc, ctx);
                self.step_index += 1;
            }
            3..=5 => {
                bus.read(0x0100 | (self.s as u16), ctx);
                self.step_index += 1;
            }
            6 => {
                self.pc = bus.read(VECTOR_RESET, ctx) as u16;
                self.step_index += 1;
            }
            7 => {
                let hi = bus.read(VECTOR_RESET + 1, ctx) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
            }
            _ => unreachable!(),
        }
    }

    fn tick_interrupt<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        match self.step_index {
            0 => {
                bus.read(self.pc, ctx);
                self.step_index += 1;
            }
            1 => {
                bus.read(self.pc, ctx);
                self.step_index += 1;
            }
            2 => {
                bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
                self.step_index += 1;
            }
            3 => {
                bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
                self.step_index += 1;
            }
            4 => {
                bus.write(0x0100 | (self.s as u16), self.p | 0x20, ctx);
                self.s = self.s.wrapping_sub(1);
                self.set_flag(2, true);
                self.step_index += 1;
            }
            5 => {
                self.pc = bus.read(self.pending_vector, ctx) as u16;
                self.step_index += 1;
            }
            6 => {
                let hi = bus.read(self.pending_vector + 1, ctx) as u16;
                self.pc |= hi << 8;
                self.state = CycleState::FetchOpcode;
                self.step_index = 0;
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.wrapping_add(1);
                }
            }
            _ => unreachable!(),
        }
    }

    fn tick_fetch<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if bus.nmi_pending() {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_NMI;
            self.step_index = 0;
            self.tick_interrupt(bus, ctx);
            return;
        }
        if bus.irq_pending() && !self.get_flag(2) {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_IRQ;
            self.step_index = 0;
            self.tick_interrupt(bus, ctx);
            return;
        }

        self.ir = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);

        #[cfg(feature = "debug")]
        if let Some(hook) = self.debug_hook {
            hook(self, self.debug_user_data);
        }

        self.state = CycleState::AddrMode;
        self.step_index = 0;
    }

    fn tick_addr<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        let instruction = &INSTRUCTION_TABLE[self.ir as usize];
        let current_slice = instruction.addr;

        if current_slice.is_empty() {
            self.state = CycleState::Operation;
            self.step_index = 0;
            self.tick_operation(bus, ctx);
            return;
        }

        if self.step_index < current_slice.len() {
            let uop = current_slice[self.step_index];
            if (uop == AddrMicroOp::OptionalFixupX
                || uop == AddrMicroOp::OptionalFixupY
                || uop == AddrMicroOp::OptionalFixupIndY)
                && !self.page_crossed
            {
                self.step_index += 1;
                if self.step_index >= current_slice.len() {
                    self.state = CycleState::Operation;
                    self.step_index = 0;
                    self.tick_operation(bus, ctx);
                    return;
                }
            } else {
                self.execute_addr_uop(uop, bus, ctx);
                self.step_index += 1;
            }
        }

        if self.step_index >= current_slice.len() {
            if instruction.exec.is_empty() {
                self.finish_instruction();
            } else {
                self.state = CycleState::Operation;
            }
            self.step_index = 0;
        }
    }

    fn tick_operation<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        let instruction = &INSTRUCTION_TABLE[self.ir as usize];
        let current_slice = instruction.exec;

        if current_slice.is_empty() {
            bus.read(self.pc, ctx);
            self.finish_instruction();
            self.step_index = 0;
            return;
        }

        if self.step_index < current_slice.len() {
            let uop = current_slice[self.step_index];
            self.execute_exec_uop(uop, bus, ctx);
            self.step_index += 1;
        }

        if self.step_index >= current_slice.len() {
            self.finish_instruction();
            self.step_index = 0;
        }
    }

    fn finish_instruction(&mut self) {
        self.state = CycleState::FetchOpcode;
    }

    fn execute_addr_uop<B: Bus>(&mut self, uop: AddrMicroOp, bus: &mut B, ctx: &mut B::Ctx) {
        match uop {
            AddrMicroOp::FetchAddrLo => {
                let lo = bus.read(self.pc, ctx);
                self.effective_addr = lo as u16;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::FetchAddrHi => {
                let hi = bus.read(self.pc, ctx);
                self.effective_addr |= (hi as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::FetchAddrHiAndJump => {
                let hi = bus.read(self.pc, ctx) as u16;
                self.pc = (hi << 8) | (self.effective_addr & 0x00FF);
            }
            AddrMicroOp::AddXToZP => {
                let lo = self.effective_addr as u8;
                bus.read(self.effective_addr, ctx);
                self.effective_addr = lo.wrapping_add(self.x) as u16;
            }
            AddrMicroOp::AddYToZP => {
                let lo = self.effective_addr as u8;
                bus.read(self.effective_addr, ctx);
                self.effective_addr = lo.wrapping_add(self.y) as u16;
            }
            AddrMicroOp::AddXToAbs => {
                let hi = bus.read(self.pc, ctx) as u16;
                self.pc = self.pc.wrapping_add(1);
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.x);
                self.effective_addr = (hi << 8) | (lo as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::AddYToAbs => {
                let hi = bus.read(self.pc, ctx) as u16;
                self.pc = self.pc.wrapping_add(1);
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.y);
                self.effective_addr = (hi << 8) | (lo as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::FetchIndXLo => {
                self.data_bus = bus.read(self.effective_addr, ctx);
                self.effective_addr =
                    (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF);
            }
            AddrMicroOp::FetchIndXHi => {
                let hi = bus.read(self.effective_addr, ctx) as u16;
                self.effective_addr = (hi << 8) | (self.data_bus as u16);
            }
            AddrMicroOp::FetchIndYLo => {
                self.data_bus = bus.read(self.effective_addr, ctx);
                self.effective_addr =
                    (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF);
            }
            AddrMicroOp::FetchIndYHi => {
                let hi = bus.read(self.effective_addr, ctx) as u16;
                let al = self.data_bus as u16;
                let (new_al, carry) = (al as u8).overflowing_add(self.y);
                self.effective_addr = (hi << 8) | (new_al as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::OptionalFixupX
            | AddrMicroOp::OptionalFixupY
            | AddrMicroOp::OptionalFixupIndY
            | AddrMicroOp::MandatoryFixupX
            | AddrMicroOp::MandatoryFixupY
            | AddrMicroOp::MandatoryFixupIndY => {
                bus.read(self.effective_addr, ctx);
                if self.page_crossed {
                    self.effective_addr = self.effective_addr.wrapping_add(0x100);
                }
            }
            AddrMicroOp::DummyReadPC => {
                bus.read(self.pc, ctx);
            }
            AddrMicroOp::DummyReadEff => {
                bus.read(self.effective_addr, ctx);
            }
        }
    }

    fn execute_exec_uop<B: Bus>(&mut self, uop: ExecMicroOp, bus: &mut B, ctx: &mut B::Ctx) {
        match uop {
            ExecMicroOp::ReadEffAndLDA => {
                self.a = bus.read(self.effective_addr, ctx);
                self.update_zn(self.a);
            }
            ExecMicroOp::ReadEffAndLDX => {
                self.x = bus.read(self.effective_addr, ctx);
                self.update_zn(self.x);
            }
            ExecMicroOp::ReadEffAndLDY => {
                self.y = bus.read(self.effective_addr, ctx);
                self.update_zn(self.y);
            }
            ExecMicroOp::ReadEffAndADC => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_adc(val);
            }
            ExecMicroOp::ReadEffAndSBC => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_sbc(val);
            }
            ExecMicroOp::ReadEffAndAND => {
                let val = bus.read(self.effective_addr, ctx);
                self.a &= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::ReadEffAndORA => {
                let val = bus.read(self.effective_addr, ctx);
                self.a |= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::ReadEffAndEOR => {
                let val = bus.read(self.effective_addr, ctx);
                self.a ^= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::ReadEffAndBIT => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_bit(val);
            }
            ExecMicroOp::ReadEffAndCMP => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_compare(self.a, val);
            }
            ExecMicroOp::ReadEffAndCPX => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_compare(self.x, val);
            }
            ExecMicroOp::ReadEffAndCPY => {
                let val = bus.read(self.effective_addr, ctx);
                self.do_compare(self.y, val);
            }
            ExecMicroOp::FetchImmAndLDA => {
                let val = self.fetch_imm(bus, ctx);
                self.a = val;
                self.update_zn(self.a);
            }
            ExecMicroOp::FetchImmAndLDX => {
                let val = self.fetch_imm(bus, ctx);
                self.x = val;
                self.update_zn(self.x);
            }
            ExecMicroOp::FetchImmAndLDY => {
                let val = self.fetch_imm(bus, ctx);
                self.y = val;
                self.update_zn(self.y);
            }
            ExecMicroOp::FetchImmAndADC => {
                let val = self.fetch_imm(bus, ctx);
                self.do_adc(val);
            }
            ExecMicroOp::FetchImmAndSBC => {
                let val = self.fetch_imm(bus, ctx);
                self.do_sbc(val);
            }
            ExecMicroOp::FetchImmAndAND => {
                let val = self.fetch_imm(bus, ctx);
                self.a &= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::FetchImmAndORA => {
                let val = self.fetch_imm(bus, ctx);
                self.a |= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::FetchImmAndEOR => {
                let val = self.fetch_imm(bus, ctx);
                self.a ^= val;
                self.update_zn(self.a);
            }
            ExecMicroOp::FetchImmAndCMP => {
                let val = self.fetch_imm(bus, ctx);
                self.do_compare(self.a, val);
            }
            ExecMicroOp::FetchImmAndCPX => {
                let val = self.fetch_imm(bus, ctx);
                self.do_compare(self.x, val);
            }
            ExecMicroOp::FetchImmAndCPY => {
                let val = self.fetch_imm(bus, ctx);
                self.do_compare(self.y, val);
            }
            ExecMicroOp::WriteEffA => {
                bus.write(self.effective_addr, self.a, ctx);
            }
            ExecMicroOp::WriteEffX => {
                bus.write(self.effective_addr, self.x, ctx);
            }
            ExecMicroOp::WriteEffY => {
                bus.write(self.effective_addr, self.y, ctx);
            }

            ExecMicroOp::ReadEffToData => {
                self.temp_data = bus.read(self.effective_addr, ctx);
            }
            ExecMicroOp::WriteDataBack => {
                bus.write(self.effective_addr, self.temp_data, ctx);
            }
            ExecMicroOp::ASLWrite => {
                let carry = (self.temp_data & 0x80) != 0;
                let res = self.temp_data << 1;
                self.set_flag(0, carry);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }
            ExecMicroOp::LSRWrite => {
                let carry = (self.temp_data & 0x01) != 0;
                let res = self.temp_data >> 1;
                self.set_flag(0, carry);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }
            ExecMicroOp::ROLWrite => {
                let carry = (self.temp_data & 0x80) != 0;
                let res = (self.temp_data << 1) | (if self.get_flag(0) { 1 } else { 0 });
                self.set_flag(0, carry);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }
            ExecMicroOp::RORWrite => {
                let carry = (self.temp_data & 0x01) != 0;
                let res = (self.temp_data >> 1) | (if self.get_flag(0) { 0x80 } else { 0 });
                self.set_flag(0, carry);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }
            ExecMicroOp::INCWrite => {
                let res = self.temp_data.wrapping_add(1);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }
            ExecMicroOp::DECWrite => {
                let res = self.temp_data.wrapping_sub(1);
                self.update_zn(res);
                bus.write(self.effective_addr, res, ctx);
            }

            ExecMicroOp::AccASL => {
                let carry = (self.a & 0x80) != 0;
                self.a <<= 1;
                self.set_flag(0, carry);
                self.update_zn(self.a);
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::AccLSR => {
                let carry = (self.a & 0x01) != 0;
                self.a >>= 1;
                self.set_flag(0, carry);
                self.update_zn(self.a);
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::AccROL => {
                let carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | (if self.get_flag(0) { 1 } else { 0 });
                self.set_flag(0, carry);
                self.update_zn(self.a);
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::AccROR => {
                let carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | (if self.get_flag(0) { 0x80 } else { 0 });
                self.set_flag(0, carry);
                self.update_zn(self.a);
                bus.read(self.pc, ctx);
            }

            ExecMicroOp::TAX => {
                bus.read(self.pc, ctx);
                self.x = self.a;
                self.update_zn(self.x);
            }
            ExecMicroOp::TXA => {
                bus.read(self.pc, ctx);
                self.a = self.x;
                self.update_zn(self.a);
            }
            ExecMicroOp::TAY => {
                bus.read(self.pc, ctx);
                self.y = self.a;
                self.update_zn(self.y);
            }
            ExecMicroOp::TYA => {
                bus.read(self.pc, ctx);
                self.a = self.y;
                self.update_zn(self.a);
            }
            ExecMicroOp::TSX => {
                bus.read(self.pc, ctx);
                self.x = self.s;
                self.update_zn(self.x);
            }
            ExecMicroOp::TXS => {
                bus.read(self.pc, ctx);
                self.s = self.x;
            }
            ExecMicroOp::INX => {
                bus.read(self.pc, ctx);
                self.x = self.x.wrapping_add(1);
                self.update_zn(self.x);
            }
            ExecMicroOp::DEX => {
                bus.read(self.pc, ctx);
                self.x = self.x.wrapping_sub(1);
                self.update_zn(self.x);
            }
            ExecMicroOp::INY => {
                bus.read(self.pc, ctx);
                self.y = self.y.wrapping_add(1);
                self.update_zn(self.y);
            }
            ExecMicroOp::DEY => {
                bus.read(self.pc, ctx);
                self.y = self.y.wrapping_sub(1);
                self.update_zn(self.y);
            }
            ExecMicroOp::CLC => {
                bus.read(self.pc, ctx);
                self.set_flag(0, false);
            }
            ExecMicroOp::SEC => {
                bus.read(self.pc, ctx);
                self.set_flag(0, true);
            }
            ExecMicroOp::CLI => {
                bus.read(self.pc, ctx);
                self.set_flag(2, false);
            }
            ExecMicroOp::SEI => {
                bus.read(self.pc, ctx);
                self.set_flag(2, true);
            }
            ExecMicroOp::CLV => {
                bus.read(self.pc, ctx);
                self.set_flag(6, false);
            }
            ExecMicroOp::CLD => {
                bus.read(self.pc, ctx);
                self.set_flag(3, false);
            }
            ExecMicroOp::SED => {
                bus.read(self.pc, ctx);
                self.set_flag(3, true);
            }

            ExecMicroOp::JumpIndCycle4 => {
                self.data_bus = bus.read(self.effective_addr, ctx);
            }
            ExecMicroOp::JumpIndCycle5 => {
                let lo = self.data_bus;
                let hi_addr =
                    (self.effective_addr & 0xFF00) | (self.effective_addr.wrapping_add(1) & 0x00FF);
                let hi = bus.read(hi_addr, ctx);
                self.pc = (hi as u16) << 8 | (lo as u16);
            }
            ExecMicroOp::BranchCycle2 => {
                let offset_u8 = bus.read(self.pc, ctx);
                let offset = offset_u8 as i8;
                self.pc = self.pc.wrapping_add(1);
                let condition = match self.ir {
                    0x10 => !self.get_flag(7),
                    0x30 => self.get_flag(7),
                    0x50 => !self.get_flag(6),
                    0x70 => self.get_flag(6),
                    0x90 => !self.get_flag(0),
                    0xB0 => self.get_flag(0),
                    0xD0 => !self.get_flag(1),
                    0xF0 => self.get_flag(1),
                    _ => unreachable!(),
                };
                if condition {
                    self.effective_addr = self.pc.wrapping_add(offset as u16);
                } else {
                    self.step_index = 2;
                }
            }
            ExecMicroOp::BranchCycle3 => {
                bus.read((self.pc & 0xFF00) | (self.effective_addr & 0x00FF), ctx);
                if (self.pc & 0xFF00) == (self.effective_addr & 0xFF00) {
                    self.pc = self.effective_addr;
                    self.step_index = 2;
                }
            }
            ExecMicroOp::BranchCycle4 => {
                bus.read(self.effective_addr, ctx);
                self.pc = self.effective_addr;
            }
            ExecMicroOp::JsrCycle2 => {
                self.data_bus = bus.read(self.pc, ctx);
                self.pc = self.pc.wrapping_add(1);
            }
            ExecMicroOp::JsrCycle3 => {
                bus.read(0x0100 | (self.s as u16), ctx);
            }
            ExecMicroOp::JsrCycle4 => {
                bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::JsrCycle5 => {
                bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::JsrCycle6 => {
                let hi = bus.read(self.pc, ctx) as u16;
                self.pc = (hi << 8) | (self.data_bus as u16);
            }
            ExecMicroOp::RtsCycle2 => {
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::RtsCycle3 => {
                bus.read(0x0100 | (self.s as u16), ctx);
            }
            ExecMicroOp::RtsCycle4 => {
                self.s = self.s.wrapping_add(1);
                self.pc = (self.pc & 0xFF00) | (bus.read(0x0100 | (self.s as u16), ctx) as u16);
            }
            ExecMicroOp::RtsCycle5 => {
                self.s = self.s.wrapping_add(1);
                self.pc =
                    ((bus.read(0x0100 | (self.s as u16), ctx) as u16) << 8) | (self.pc & 0x00FF);
            }
            ExecMicroOp::RtsCycle6 => {
                bus.read(self.pc, ctx);
                self.pc = self.pc.wrapping_add(1);
            }
            ExecMicroOp::RtiCycle2 => {
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::RtiCycle3 => {
                bus.read(0x0100 | (self.s as u16), ctx);
            }
            ExecMicroOp::RtiCycle4 => {
                self.s = self.s.wrapping_add(1);
                let p = bus.read(0x0100 | (self.s as u16), ctx);
                self.p = (p & 0xCF) | 0x20;
            }
            ExecMicroOp::RtiCycle5 => {
                self.s = self.s.wrapping_add(1);
                self.pc = (self.pc & 0xFF00) | (bus.read(0x0100 | (self.s as u16), ctx) as u16);
            }
            ExecMicroOp::RtiCycle6 => {
                self.s = self.s.wrapping_add(1);
                self.pc =
                    ((bus.read(0x0100 | (self.s as u16), ctx) as u16) << 8) | (self.pc & 0x00FF);
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.saturating_sub(1);
                }
            }

            ExecMicroOp::BrkCycle2 => {
                bus.read(self.pc, ctx);
                self.pc = self.pc.wrapping_add(1);
            }
            ExecMicroOp::BrkCycle3 => {
                bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::BrkCycle4 => {
                bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::BrkCycle5 => {
                bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::BrkCycle6 => {
                self.pc = bus.read(VECTOR_IRQ, ctx) as u16;
                self.set_flag(2, true);
            }
            ExecMicroOp::BrkCycle7 => {
                let hi = bus.read(VECTOR_IRQ + 1, ctx) as u16;
                self.pc |= hi << 8;
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.wrapping_add(1);
                }
            }

            ExecMicroOp::PushA => {
                bus.write(0x0100 | (self.s as u16), self.a, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::PushP => {
                bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::PushPCH => {
                bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::PushPCL => {
                bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx);
                self.s = self.s.wrapping_sub(1);
            }
            ExecMicroOp::PullA => {
                self.s = self.s.wrapping_add(1);
                self.a = bus.read(0x0100 | (self.s as u16), ctx);
                self.update_zn(self.a);
            }
            ExecMicroOp::PullP => {
                self.s = self.s.wrapping_add(1);
                let p = bus.read(0x0100 | (self.s as u16), ctx);
                self.p = (p & 0xCF) | 0x20;
            }
            ExecMicroOp::PullPCH => {
                self.s = self.s.wrapping_add(1);
                let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16;
                self.pc = (hi << 8) | (self.pc & 0x00FF);
            }
            ExecMicroOp::PullPCL => {
                self.s = self.s.wrapping_add(1);
                let lo = bus.read(0x0100 | (self.s as u16), ctx) as u16;
                self.pc = (self.pc & 0xFF00) | lo;
            }

            ExecMicroOp::DummyReadStack => {
                bus.read(0x0100 | (self.s as u16), ctx);
            }
            ExecMicroOp::DummyReadPC => {
                bus.read(self.pc, ctx);
            }
            ExecMicroOp::DummyReadEff => {
                bus.read(self.effective_addr, ctx);
            }
            ExecMicroOp::Unimplemented => {
                bus.read(self.pc, ctx);
            }
        }
    }

    #[inline(always)]
    fn fetch_imm<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        let val = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn update_zn(&mut self, val: u8) {
        self.set_flag(1, val == 0);
        self.set_flag(7, (val & 0x80) != 0);
    }

    fn set_flag(&mut self, bit: u8, val: bool) {
        if bit == 4 || bit == 5 {
            return;
        }
        if val {
            self.p |= 1 << bit;
        } else {
            self.p &= !(1 << bit);
        }
    }

    fn get_flag(&self, bit: u8) -> bool {
        (self.p & (1 << bit)) != 0
    }

    fn do_compare(&mut self, reg: u8, val: u8) {
        let (res, borrow) = reg.overflowing_sub(val);
        self.set_flag(0, !borrow);
        self.update_zn(res);
    }

    fn do_adc(&mut self, val: u8) {
        let a = self.a as u16;
        let b = val as u16;
        let c = if self.get_flag(0) { 1 } else { 0 };
        let sum = a + b + c;

        if self.get_flag(3) {
            let mut low = (a & 0x0F) + (b & 0x0F) + c;
            if low > 9 {
                low += 6;
            }
            let mut high = (a >> 4) + (b >> 4) + (if low > 15 { 1 } else { 0 });
            self.set_flag(0, sum > 0xFF);
            self.update_zn((sum & 0xFF) as u8);
            self.set_flag(6, (!(a ^ b) & (a ^ sum) & 0x80) != 0);
            if high > 9 {
                high += 6;
            }
            self.set_flag(0, high > 15);
            self.a = ((high << 4) | (low & 0x0F)) as u8;
        } else {
            self.set_flag(0, sum > 0xFF);
            let res8 = (sum & 0xFF) as u8;
            self.set_flag(6, (!(a ^ b) & (a ^ sum) & 0x80) != 0);
            self.update_zn(res8);
            self.a = res8;
        }
    }

    fn do_sbc(&mut self, val: u8) {
        let a = self.a as i16;
        let b = val as i16;
        let c = if self.get_flag(0) { 1 } else { 0 };
        let diff = a - b - (1 - c);

        if self.get_flag(3) {
            let mut low = (a & 0x0F) - (b & 0x0F) - (1 - c);
            if low < 0 {
                low -= 6;
            }
            let mut high = (a >> 4) - (b >> 4) - (if low < 0 { 1 } else { 0 });
            if high < 0 {
                high -= 6;
            }
            self.set_flag(0, diff >= 0);
            self.update_zn((diff & 0xFF) as u8);
            self.set_flag(6, ((a ^ diff) & (a ^ b) & 0x80) != 0);
            self.a = ((high as u16) << 4 | (low as u16 & 0x0F)) as u8;
        } else {
            self.set_flag(0, diff >= 0);
            let res8 = (diff & 0xFF) as u8;
            self.set_flag(6, ((a ^ diff) & (a ^ b) & 0x80) != 0);
            self.update_zn(res8);
            self.a = res8;
        }
    }

    fn do_bit(&mut self, val: u8) {
        self.set_flag(1, (self.a & val) == 0);
        self.set_flag(7, (val & 0x80) != 0);
        self.set_flag(6, (val & 0x40) != 0);
    }
}
