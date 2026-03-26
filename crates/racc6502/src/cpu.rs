use crate::bus::Bus;
use crate::instructions::{AddrMicroOp, ExecMicroOp, VECTOR_RESET, VECTOR_NMI, VECTOR_IRQ};
use crate::instructions::nmos6502::INSTRUCTION_TABLE;

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

/// A hook that is called at the end of every instruction (the "Sync" point).
/// 
/// This type is only available when `debug_assertions` are enabled.
/// The second argument is a pointer to optional user data.
#[cfg(feature = "debug")]
pub type DebugHook<B> = fn(&CPU<B>, *mut core::ffi::c_void);

/// The cycle-accurate MOS 6502 CPU core.
pub struct CPU<B: Bus> {
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
    /// The system bus used for all memory and peripheral access.
    pub bus: B,

    /// An optional hook called at the end of every instruction.
    #[cfg(feature = "debug")]
    pub debug_hook: Option<DebugHook<B>>,
    /// Optional user data pointer passed to the debug hook.
    #[cfg(feature = "debug")]
    pub debug_user_data: *mut core::ffi::c_void,
    /// The current interrupt nesting level (0 = main thread).
    #[cfg(feature = "debug")]
    pub debug_interrupt_level: u8,
}

impl<B: Bus> CPU<B> {
    /// Creates a new CPU instance attached to the provided bus.
    /// 
    /// The CPU starts in the `Reset` state.
    pub fn new(bus: B) -> Self {
        let mut cpu = CPU {
            a: 0,
            x: 0,
            y: 0,
            pc: 0,
            s: 0xFD,
            p: 0x24, // Bits 5 and 2 (Interrupt Disable) set
            ir: 0,
            step_index: 0,
            state: CycleState::Reset,
            effective_addr: 0,
            data_bus: 0,
            temp_data: 0,
            page_crossed: false,
            pending_vector: 0,
            bus,
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
    /// 
    /// On the next call to `tick()`, the CPU will begin the hardware-standard
    /// 7-cycle reset sequence, eventually fetching the start address from `$FFFC`.
    pub fn reset(&mut self) {
        self.state = CycleState::Reset;
        self.step_index = 0;
    }

    /// Executes exactly one clock cycle of the CPU.
    /// 
    /// This method performs exactly one bus access (read or write) and updates
    /// the internal state of the CPU.
    pub fn tick(&mut self) {
        // Sample the Set Overflow pin transition
        if self.bus.so_pending() {
            self.set_flag(6, true);
        }

        match self.state {
            CycleState::Reset => self.tick_reset(),
            CycleState::Interrupt => self.tick_interrupt(),
            CycleState::FetchOpcode => self.tick_fetch(),
            CycleState::AddrMode => self.tick_addr(),
            CycleState::Operation => self.tick_operation(),
        }
    }

    /// Progresses the 7-cycle reset sequence.
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

    /// Progresses the 7-cycle interrupt sequence (IRQ/NMI).
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
                // Bit 4 (B) is 0 for hardware interrupts
                self.bus.write(0x0100 | (self.s as u16), self.p | 0x20); 
                self.s = self.s.wrapping_sub(1);
                self.set_flag(2, true); // Set Interrupt Disable
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
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.wrapping_add(1);
                }
            }
            _ => unreachable!(),
        }
    }

    /// Fetches the next opcode and checks for pending interrupts.
    fn tick_fetch(&mut self) {
        // :? Is there a debug hook present?
        #[cfg(feature = "debug")]
        if let Some(hook) = self.debug_hook {
            // -> Yes, then call the hook with the current CPU state and user data pointer
            hook(self, self.debug_user_data);
        }

        if self.bus.nmi_pending() {
            self.state = CycleState::Interrupt;
            self.pending_vector = VECTOR_NMI;
            self.step_index = 0;
            self.tick_interrupt();
            return;
        }
        if self.bus.irq_pending() && !self.get_flag(2) {
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

    /// Executes one cycle of the addressing mode phase.
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
            // Handle optional page-cross fixup cycles
            if (uop == AddrMicroOp::OptionalFixupX || uop == AddrMicroOp::OptionalFixupY || uop == AddrMicroOp::OptionalFixupIndY) && !self.page_crossed {
                self.step_index += 1;
                if self.step_index >= current_slice.len() {
                    self.state = CycleState::Operation;
                    self.step_index = 0;
                    self.tick_operation();
                    return;
                }
            } else {
                self.execute_addr_uop(uop);
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

    /// Executes one cycle of the operation phase.
    fn tick_operation(&mut self) {
        let instruction = &INSTRUCTION_TABLE[self.ir as usize];
        let current_slice = instruction.exec;

        if current_slice.is_empty() {
            self.bus.read(self.pc); // Mandatory cycle
            self.finish_instruction();
            self.step_index = 0;
            return;
        }

        if self.step_index < current_slice.len() {
            let uop = current_slice[self.step_index];
            self.execute_exec_uop(uop);
            self.step_index += 1;
        }

        if self.step_index >= current_slice.len() {
            self.finish_instruction();
            self.step_index = 0;
        }
    }

    /// Completes the current instruction and invokes the debug hook if present.
    fn finish_instruction(&mut self) {
        self.state = CycleState::FetchOpcode;
    }

    /// Maps a single `AddrMicroOp` to its cycle-accurate bus and state logic.
    fn execute_addr_uop(&mut self, uop: AddrMicroOp) {
        match uop {
            AddrMicroOp::FetchAddrLo => {
                let lo = self.bus.read(self.pc);
                self.effective_addr = lo as u16;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::FetchAddrHi => {
                let hi = self.bus.read(self.pc);
                self.effective_addr |= (hi as u16) << 8;
                self.pc = self.pc.wrapping_add(1);
            }
            AddrMicroOp::FetchAddrHiAndJump => {
                let hi = self.bus.read(self.pc) as u16;
                self.pc = (hi << 8) | (self.effective_addr & 0x00FF);
            }
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
                let hi = self.bus.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.x);
                self.effective_addr = (hi << 8) | (lo as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::AddYToAbs => {
                let hi = self.bus.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let (lo, carry) = (self.effective_addr as u8).overflowing_add(self.y);
                self.effective_addr = (hi << 8) | (lo as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::FetchIndXLo => {
                self.data_bus = self.bus.read(self.effective_addr);
                self.effective_addr = (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF);
            }
            AddrMicroOp::FetchIndXHi => {
                let hi = self.bus.read(self.effective_addr) as u16;
                self.effective_addr = (hi << 8) | (self.data_bus as u16);
            }
            AddrMicroOp::FetchIndYLo => {
                self.data_bus = self.bus.read(self.effective_addr);
                self.effective_addr = (self.effective_addr & 0xFF00) | ((self.effective_addr + 1) & 0x00FF);
            }
            AddrMicroOp::FetchIndYHi => {
                let hi = self.bus.read(self.effective_addr) as u16;
                let al = self.data_bus as u16;
                let (new_al, carry) = (al as u8).overflowing_add(self.y);
                self.effective_addr = (hi << 8) | (new_al as u16);
                self.page_crossed = carry;
            }
            AddrMicroOp::OptionalFixupX | AddrMicroOp::OptionalFixupY | AddrMicroOp::OptionalFixupIndY |
            AddrMicroOp::MandatoryFixupX | AddrMicroOp::MandatoryFixupY | AddrMicroOp::MandatoryFixupIndY => {
                self.bus.read(self.effective_addr); // Dummy read from wrong page
                if self.page_crossed {
                    self.effective_addr = self.effective_addr.wrapping_add(0x100);
                }
            }
            AddrMicroOp::DummyReadPC => { self.bus.read(self.pc); }
            AddrMicroOp::DummyReadEff => { self.bus.read(self.effective_addr); }
        }
    }

    /// Maps a single `ExecMicroOp` to its cycle-accurate bus and state logic.
    fn execute_exec_uop(&mut self, uop: ExecMicroOp) {
        match uop {
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
            ExecMicroOp::FetchImmAndLDA => { let val = self.fetch_imm(); self.a = val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndLDX => { let val = self.fetch_imm(); self.x = val; self.update_zn(self.x); }
            ExecMicroOp::FetchImmAndLDY => { let val = self.fetch_imm(); self.y = val; self.update_zn(self.y); }
            ExecMicroOp::FetchImmAndADC => { let val = self.fetch_imm(); self.do_adc(val); }
            ExecMicroOp::FetchImmAndSBC => { let val = self.fetch_imm(); self.do_sbc(val); }
            ExecMicroOp::FetchImmAndAND => { let val = self.fetch_imm(); self.a &= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndORA => { let val = self.fetch_imm(); self.a |= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndEOR => { let val = self.fetch_imm(); self.a ^= val; self.update_zn(self.a); }
            ExecMicroOp::FetchImmAndCMP => { let val = self.fetch_imm(); self.do_compare(self.a, val); }
            ExecMicroOp::FetchImmAndCPX => { let val = self.fetch_imm(); self.do_compare(self.x, val); }
            ExecMicroOp::FetchImmAndCPY => { let val = self.fetch_imm(); self.do_compare(self.y, val); }
            ExecMicroOp::WriteEffA => { self.bus.write(self.effective_addr, self.a); }
            ExecMicroOp::WriteEffX => { self.bus.write(self.effective_addr, self.x); }
            ExecMicroOp::WriteEffY => { self.bus.write(self.effective_addr, self.y); }

            // RMW
            ExecMicroOp::ReadEffToData => { self.temp_data = self.bus.read(self.effective_addr); }
            ExecMicroOp::WriteDataBack => { self.bus.write(self.effective_addr, self.temp_data); }
            ExecMicroOp::ASLWrite => {
                let carry = (self.temp_data & 0x80) != 0;
                let res = self.temp_data << 1;
                self.set_flag(0, carry); self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }
            ExecMicroOp::LSRWrite => {
                let carry = (self.temp_data & 0x01) != 0;
                let res = self.temp_data >> 1;
                self.set_flag(0, carry); self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }
            ExecMicroOp::ROLWrite => {
                let carry = (self.temp_data & 0x80) != 0;
                let res = (self.temp_data << 1) | (if self.get_flag(0) { 1 } else { 0 });
                self.set_flag(0, carry); self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }
            ExecMicroOp::RORWrite => {
                let carry = (self.temp_data & 0x01) != 0;
                let res = (self.temp_data >> 1) | (if self.get_flag(0) { 0x80 } else { 0 });
                self.set_flag(0, carry); self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }
            ExecMicroOp::INCWrite => {
                let res = self.temp_data.wrapping_add(1);
                self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }
            ExecMicroOp::DECWrite => {
                let res = self.temp_data.wrapping_sub(1);
                self.update_zn(res);
                self.bus.write(self.effective_addr, res);
            }

            ExecMicroOp::AccASL => {
                let carry = (self.a & 0x80) != 0;
                self.a <<= 1;
                self.set_flag(0, carry); self.update_zn(self.a);
                self.bus.read(self.pc);
            }
            ExecMicroOp::AccLSR => {
                let carry = (self.a & 0x01) != 0;
                self.a >>= 1;
                self.set_flag(0, carry); self.update_zn(self.a);
                self.bus.read(self.pc);
            }
            ExecMicroOp::AccROL => {
                let carry = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | (if self.get_flag(0) { 1 } else { 0 });
                self.set_flag(0, carry); self.update_zn(self.a);
                self.bus.read(self.pc);
            }
            ExecMicroOp::AccROR => {
                let carry = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | (if self.get_flag(0) { 0x80 } else { 0 });
                self.set_flag(0, carry); self.update_zn(self.a);
                self.bus.read(self.pc);
            }

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
            
            ExecMicroOp::JumpIndCycle4 => { self.data_bus = self.bus.read(self.effective_addr); }
            ExecMicroOp::JumpIndCycle5 => {
                let lo = self.data_bus;
                let hi_addr = (self.effective_addr & 0xFF00) | (self.effective_addr.wrapping_add(1) & 0x00FF);
                let hi = self.bus.read(hi_addr);
                self.pc = (hi as u16) << 8 | (lo as u16);
            }
            ExecMicroOp::BranchCycle2 => {
                let offset_u8 = self.bus.read(self.pc);
                let offset = offset_u8 as i8;
                self.pc = self.pc.wrapping_add(1);
                let condition = match self.ir {
                    0x10 => !self.get_flag(7), 0x30 => self.get_flag(7),
                    0x50 => !self.get_flag(6), 0x70 => self.get_flag(6),
                    0x90 => !self.get_flag(0), 0xB0 => self.get_flag(0),
                    0xD0 => !self.get_flag(1), 0xF0 => self.get_flag(1),
                    _ => unreachable!(),
                };
                if condition {
                    self.effective_addr = self.pc.wrapping_add(offset as u16);
                } else {
                    self.step_index = 2; // Finish
                }
            }
            ExecMicroOp::BranchCycle3 => {
                self.bus.read((self.pc & 0xFF00) | (self.effective_addr & 0x00FF));
                if (self.pc & 0xFF00) == (self.effective_addr & 0xFF00) {
                    self.pc = self.effective_addr;
                    self.step_index = 2; // Finish
                }
            }
            ExecMicroOp::BranchCycle4 => {
                self.bus.read(self.effective_addr);
                self.pc = self.effective_addr;
            }
            ExecMicroOp::JsrCycle2 => { self.data_bus = self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::JsrCycle3 => { self.bus.read(0x0100 | (self.s as u16)); }
            ExecMicroOp::JsrCycle4 => { self.bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::JsrCycle5 => { self.bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::JsrCycle6 => { let hi = self.bus.read(self.pc) as u16; self.pc = (hi << 8) | (self.data_bus as u16); }
            ExecMicroOp::RtsCycle2 => { self.bus.read(self.pc); }
            ExecMicroOp::RtsCycle3 => { self.bus.read(0x0100 | (self.s as u16)); }
            ExecMicroOp::RtsCycle4 => { self.s = self.s.wrapping_add(1); self.pc = (self.pc & 0xFF00) | (self.bus.read(0x0100 | (self.s as u16)) as u16); }
            ExecMicroOp::RtsCycle5 => { self.s = self.s.wrapping_add(1); self.pc = ((self.bus.read(0x0100 | (self.s as u16)) as u16) << 8) | (self.pc & 0x00FF); }
            ExecMicroOp::RtsCycle6 => { self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::RtiCycle2 => { self.bus.read(self.pc); }
            ExecMicroOp::RtiCycle3 => { self.bus.read(0x0100 | (self.s as u16)); }
            ExecMicroOp::RtiCycle4 => { self.s = self.s.wrapping_add(1); let p = self.bus.read(0x0100 | (self.s as u16)); self.p = (p & 0xCF) | 0x20; }
            ExecMicroOp::RtiCycle5 => { self.s = self.s.wrapping_add(1); self.pc = (self.pc & 0xFF00) | (self.bus.read(0x0100 | (self.s as u16)) as u16); }
            ExecMicroOp::RtiCycle6 => { 
                self.s = self.s.wrapping_add(1); 
                self.pc = ((self.bus.read(0x0100 | (self.s as u16)) as u16) << 8) | (self.pc & 0x00FF);
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.saturating_sub(1);
                }
            }

            ExecMicroOp::BrkCycle2 => { self.bus.read(self.pc); self.pc = self.pc.wrapping_add(1); }
            ExecMicroOp::BrkCycle3 => { self.bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::BrkCycle4 => { self.bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::BrkCycle5 => { self.bus.write(0x0100 | (self.s as u16), self.p | 0x30); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::BrkCycle6 => { self.pc = self.bus.read(VECTOR_IRQ) as u16; self.set_flag(2, true); }
            ExecMicroOp::BrkCycle7 => { 
                let hi = self.bus.read(VECTOR_IRQ + 1) as u16; 
                self.pc |= hi << 8; 
                #[cfg(feature = "debug")]
                {
                    self.debug_interrupt_level = self.debug_interrupt_level.wrapping_add(1);
                }
            }

            ExecMicroOp::PushA => { self.bus.write(0x0100 | (self.s as u16), self.a); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushP => { self.bus.write(0x0100 | (self.s as u16), self.p | 0x30); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushPCH => { self.bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PushPCL => { self.bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8); self.s = self.s.wrapping_sub(1); }
            ExecMicroOp::PullA => { self.s = self.s.wrapping_add(1); self.a = self.bus.read(0x0100 | (self.s as u16)); self.update_zn(self.a); }
            ExecMicroOp::PullP => { self.s = self.s.wrapping_add(1); let p = self.bus.read(0x0100 | (self.s as u16)); self.p = (p & 0xCF) | 0x20; }
            ExecMicroOp::PullPCH => { self.s = self.s.wrapping_add(1); let hi = self.bus.read(0x0100 | (self.s as u16)) as u16; self.pc = (hi << 8) | (self.pc & 0x00FF); }
            ExecMicroOp::PullPCL => { self.s = self.s.wrapping_add(1); let lo = self.bus.read(0x0100 | (self.s as u16)) as u16; self.pc = (self.pc & 0xFF00) | lo; }
            
            ExecMicroOp::DummyReadStack => { self.bus.read(0x0100 | (self.s as u16)); }
            ExecMicroOp::DummyReadPC => { self.bus.read(self.pc); }
            ExecMicroOp::DummyReadEff => { self.bus.read(self.effective_addr); }
            ExecMicroOp::Unimplemented => { self.bus.read(self.pc); }
        }
    }

    /// Helper for immediate-mode instructions that fetches the operand,
    /// advances PC, and captures the operand for debugging.
    #[inline(always)]
    fn fetch_imm(&mut self) -> u8 {
        let val = self.bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    /// Updates the Zero (Z) and Negative (N) flags based on the provided value.
    fn update_zn(&mut self, val: u8) { 
        self.set_flag(1, val == 0); 
        self.set_flag(7, (val & 0x80) != 0); 
    }

    /// Sets or clears a specific flag bit in the status register.
    /// 
    /// Note: Bits 4 (B) and 5 are not stored in the `p` register directly
    /// but are generated during stack pushes.
    fn set_flag(&mut self, bit: u8, val: bool) {
        if bit == 4 || bit == 5 { return; }
        if val { self.p |= 1 << bit; } else { self.p &= !(1 << bit); }
    }

    /// Returns the current state of a specific flag bit.
    fn get_flag(&self, bit: u8) -> bool { (self.p & (1 << bit)) != 0 }

    /// Performs a comparison between a register and a value, updating N, Z, and C flags.
    fn do_compare(&mut self, reg: u8, val: u8) { 
        let (res, borrow) = reg.overflowing_sub(val); 
        self.set_flag(0, !borrow); 
        self.update_zn(res); 
    }
    
    /// Performs an Addition with Carry, handling decimal mode if the D flag is set.
    fn do_adc(&mut self, val: u8) {
        let a = self.a as u16;
        let b = val as u16;
        let c = if self.get_flag(0) { 1 } else { 0 };
        let sum = a + b + c;

        if self.get_flag(3) {
            let mut low = (a & 0x0F) + (b & 0x0F) + c;
            if low > 9 { low += 6; }
            let mut high = (a >> 4) + (b >> 4) + (if low > 15 { 1 } else { 0 });
            
            // Flags are binary based on NMOS 6502
            self.set_flag(0, sum > 0xFF);
            self.update_zn((sum & 0xFF) as u8);
            self.set_flag(6, (!(a ^ b) & (a ^ sum) & 0x80) != 0);

            if high > 9 { high += 6; }
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

    /// Performs a Subtraction with Borrow, handling decimal mode if the D flag is set.
    fn do_sbc(&mut self, val: u8) {
        let a = self.a as i16;
        let b = val as i16;
        let c = if self.get_flag(0) { 1 } else { 0 };
        let diff = a - b - (1 - c);

        if self.get_flag(3) {
            let mut low = (a & 0x0F) - (b & 0x0F) - (1 - c);
            if low < 0 { low -= 6; }
            let mut high = (a >> 4) - (b >> 4) - (if low < 0 { 1 } else { 0 });
            if high < 0 { high -= 6; }
            
            // Flags are binary based on NMOS 6502
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

    /// Performs a bitwise AND between the accumulator and memory, updating flags.
    fn do_bit(&mut self, val: u8) { 
        self.set_flag(1, (self.a & val) == 0); 
        self.set_flag(7, (val & 0x80) != 0); 
        self.set_flag(6, (val & 0x40) != 0); 
    }
}
