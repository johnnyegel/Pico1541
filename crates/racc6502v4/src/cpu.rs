use crate::bus::Bus;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interrupt {
    Reset,
    Irq,
}

pub struct CPU {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    pub pending_interrupt: Option<Interrupt>,
    pub pending_so: bool,
    last_so_pin: bool,
    pub cycles: u32,
    pub instr_ptr: *const u8,
}

pub const FLAG_C: u8 = 0x01;
pub const FLAG_Z: u8 = 0x02;
pub const FLAG_I: u8 = 0x04;
pub const FLAG_D: u8 = 0x08;
pub const FLAG_B: u8 = 0x10;
pub const FLAG_U: u8 = 0x20;
pub const FLAG_V: u8 = 0x40;
pub const FLAG_N: u8 = 0x80;

impl Default for CPU {
    fn default() -> Self {
        Self::new()
    }
}

impl CPU {
    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, offset: u32, addr: u16) -> u8 {
        bus.read(ctx, addr, self.cycles.wrapping_add(offset))
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn write<B: Bus>(
        &mut self,
        bus: &mut B,
        ctx: &mut B::Ctx,
        offset: u32,
        addr: u16,
        val: u8,
    ) {
        bus.write(ctx, addr, val, self.cycles.wrapping_add(offset));
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn read_pc<B: Bus>(&mut self, _bus: &mut B, _ctx: &mut B::Ctx) -> u8 {
        let val = unsafe { *self.instr_ptr };
        self.instr_ptr = unsafe { self.instr_ptr.add(1) };
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn read_word<B: Bus>(
        &mut self,
        bus: &mut B,
        ctx: &mut B::Ctx,
        offset: u32,
        addr: u16,
    ) -> u16 {
        let lo = self.read(bus, ctx, offset, addr) as u16;
        let hi = self.read(bus, ctx, offset + 1, addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn read_word_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.read_pc(bus, ctx) as u16;
        let hi = self.read_pc(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn push<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, offset: u32, val: u8) {
        self.write(bus, ctx, offset, 0x0100 | (self.s as u16), val);
        self.s = self.s.wrapping_sub(1);
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn pop<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, offset: u32) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.read(bus, ctx, offset, 0x0100 | (self.s as u16))
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn push_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, offset: u32, val: u16) {
        self.push(bus, ctx, offset, (val >> 8) as u8);
        self.push(bus, ctx, offset + 1, val as u8);
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn pop_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, offset: u32) -> u16 {
        let lo = self.pop(bus, ctx, offset) as u16;
        let hi = self.pop(bus, ctx, offset + 1) as u16;
        (hi << 8) | lo
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn set_z(&mut self, val: u8) {
        if val == 0 {
            self.p |= FLAG_Z;
        } else {
            self.p &= !FLAG_Z;
        }
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn set_n(&mut self, val: u8) {
        if val & 0x80 != 0 {
            self.p |= FLAG_N;
        } else {
            self.p &= !FLAG_N;
        }
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn set_zn(&mut self, val: u8) {
        self.set_z(val);
        self.set_n(val);
    }
}

impl CPU {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            s: 0xFD,
            pc: 0,
            p: 0x24,
            pending_interrupt: None,
            pending_so: false,
            last_so_pin: false,
            cycles: 0,
            instr_ptr: core::ptr::null(),
        }
    }

    pub fn reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        let offset = 0;
        self.pc = self.read_word(bus, ctx, offset, 0xFFFC);
        self.s = self.s.wrapping_sub(3);
        self.p |= FLAG_I;
    }
}

impl CPU {
    #[link_section = ".data.pico1541_emulator_core"]
    pub fn set_so_pin(&mut self, active: bool) {
        if active && !self.last_so_pin {
            self.pending_so = true;
        }
        self.last_so_pin = active;
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn do_adc(&mut self, val: u8) {
        if self.p & FLAG_D != 0 {
            let a = self.a;
            let b = val;
            let c = self.p & FLAG_C;
            let mut low = (a & 0x0F) + (b & 0x0F) + c;
            if low > 9 {
                low += 6;
            }
            let mut high = (a >> 4) + (b >> 4) + (if low > 15 { 1 } else { 0 });
            let al = a as u16;
            let bl = b as u16;
            let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 {
                self.p |= FLAG_V;
            }
            if (res_bin & 0x80) != 0 {
                self.p |= FLAG_N;
            }
            if (res_bin as u8) == 0 {
                self.p |= FLAG_Z;
            }
            if high > 9 {
                high += 6;
            }
            if high > 15 {
                self.p |= FLAG_C;
            }
            self.a = (low & 0x0F) | ((high & 0x0F) << 4);
        } else {
            let a = self.a as u16;
            let b = val as u16;
            let c = (self.p & FLAG_C) as u16;
            let res = a + b + c;
            self.p &= !0xC3;
            if res > 0xFF {
                self.p |= FLAG_C;
            }
            let r = res as u8;
            if r == 0 {
                self.p |= FLAG_Z;
            }
            if (r & 0x80) != 0 {
                self.p |= FLAG_N;
            }
            if ((a ^ res) & (b ^ res) & 0x80) != 0 {
                self.p |= FLAG_V;
            }
            self.a = r;
        }
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn do_sbc(&mut self, val: u8) {
        if self.p & FLAG_D != 0 {
            let a = self.a;
            let b = val;
            let c = self.p & FLAG_C;
            let mut low = (a & 0x0F) as i32 - (b & 0x0F) as i32 - (1 - c as i32);
            let mut high = (a >> 4) as i32 - (b >> 4) as i32 - (if low < 0 { 1 } else { 0 });
            let al = a as u16;
            let bl = (!b) as u16;
            let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if res_bin > 0xFF {
                self.p |= FLAG_C;
            }
            if (res_bin as u8) == 0 {
                self.p |= FLAG_Z;
            }
            if (res_bin & 0x80) != 0 {
                self.p |= FLAG_N;
            }
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 {
                self.p |= FLAG_V;
            }
            if low < 0 {
                low -= 6;
            }
            if high < 0 {
                high -= 6;
            }
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
        } else {
            let a = self.a as u16;
            let b = (!val) as u16;
            let c = (self.p & FLAG_C) as u16;
            let res = a + b + c;
            self.p &= !0xC3;
            if res > 0xFF {
                self.p |= FLAG_C;
            }
            let r = res as u8;
            if r == 0 {
                self.p |= FLAG_Z;
            }
            if (r & 0x80) != 0 {
                self.p |= FLAG_N;
            }
            if ((a ^ res) & (b ^ res) & 0x80) != 0 {
                self.p |= FLAG_V;
            }
            self.a = r;
        }
    }

    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn step<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u32 {
        if let Some(interrupt) = self.pending_interrupt.take() {
            let offset = 0;
            match interrupt {
                Interrupt::Reset => {
                    self.pc = self.read_word(bus, ctx, offset, 0xFFFC);
                    self.s = self.s.wrapping_sub(3);
                    self.p |= FLAG_I;
                    let elapsed = 7;
                    self.cycles = self.cycles.wrapping_add(elapsed);
                    return elapsed;
                }
                Interrupt::Irq => {
                    if self.p & FLAG_I == 0 {
                        self.push_word(bus, ctx, offset, self.pc);
                        self.push(bus, ctx, offset, self.p | FLAG_U);
                        self.p |= FLAG_I;
                        self.pc = self.read_word(bus, ctx, offset, 0xFFFE);

                        let elapsed = 7;
                        self.cycles = self.cycles.wrapping_add(elapsed);
                        return elapsed;
                    }
                }
            }
        }

        if bus.irq_pending() {
            self.pending_interrupt = Some(Interrupt::Irq);
        }

        if self.pending_so {
            self.p |= FLAG_V;
            self.pending_so = false;
        }

        self.instr_ptr = bus.get_instruction_ptr(self.pc);
        let opcode = unsafe { *self.instr_ptr };
        self.instr_ptr = unsafe { self.instr_ptr.add(1) };
        self.pc = self.pc.wrapping_add(1);

        let elapsed = match opcode {
            0x00 => crate::opcodes::op_00(self, bus, ctx),
            0x01 => crate::opcodes::op_01(self, bus, ctx),
            0x02 => crate::opcodes::op_02(self, bus, ctx),
            0x03 => crate::opcodes::op_03(self, bus, ctx),
            0x04 => crate::opcodes::op_04(self, bus, ctx),
            0x05 => crate::opcodes::op_05(self, bus, ctx),
            0x06 => crate::opcodes::op_06(self, bus, ctx),
            0x07 => crate::opcodes::op_07(self, bus, ctx),
            0x08 => crate::opcodes::op_08(self, bus, ctx),
            0x09 => crate::opcodes::op_09(self, bus, ctx),
            0x0a => crate::opcodes::op_0a(self, bus, ctx),
            0x0b => crate::opcodes::op_0b(self, bus, ctx),
            0x0c => crate::opcodes::op_0c(self, bus, ctx),
            0x0d => crate::opcodes::op_0d(self, bus, ctx),
            0x0e => crate::opcodes::op_0e(self, bus, ctx),
            0x0f => crate::opcodes::op_0f(self, bus, ctx),
            0x10 => crate::opcodes::op_10(self, bus, ctx),
            0x11 => crate::opcodes::op_11(self, bus, ctx),
            0x12 => crate::opcodes::op_12(self, bus, ctx),
            0x13 => crate::opcodes::op_13(self, bus, ctx),
            0x14 => crate::opcodes::op_14(self, bus, ctx),
            0x15 => crate::opcodes::op_15(self, bus, ctx),
            0x16 => crate::opcodes::op_16(self, bus, ctx),
            0x17 => crate::opcodes::op_17(self, bus, ctx),
            0x18 => crate::opcodes::op_18(self, bus, ctx),
            0x19 => crate::opcodes::op_19(self, bus, ctx),
            0x1a => crate::opcodes::op_1a(self, bus, ctx),
            0x1b => crate::opcodes::op_1b(self, bus, ctx),
            0x1c => crate::opcodes::op_1c(self, bus, ctx),
            0x1d => crate::opcodes::op_1d(self, bus, ctx),
            0x1e => crate::opcodes::op_1e(self, bus, ctx),
            0x1f => crate::opcodes::op_1f(self, bus, ctx),
            0x20 => crate::opcodes::op_20(self, bus, ctx),
            0x21 => crate::opcodes::op_21(self, bus, ctx),
            0x22 => crate::opcodes::op_22(self, bus, ctx),
            0x23 => crate::opcodes::op_23(self, bus, ctx),
            0x24 => crate::opcodes::op_24(self, bus, ctx),
            0x25 => crate::opcodes::op_25(self, bus, ctx),
            0x26 => crate::opcodes::op_26(self, bus, ctx),
            0x27 => crate::opcodes::op_27(self, bus, ctx),
            0x28 => crate::opcodes::op_28(self, bus, ctx),
            0x29 => crate::opcodes::op_29(self, bus, ctx),
            0x2a => crate::opcodes::op_2a(self, bus, ctx),
            0x2b => crate::opcodes::op_2b(self, bus, ctx),
            0x2c => crate::opcodes::op_2c(self, bus, ctx),
            0x2d => crate::opcodes::op_2d(self, bus, ctx),
            0x2e => crate::opcodes::op_2e(self, bus, ctx),
            0x2f => crate::opcodes::op_2f(self, bus, ctx),
            0x30 => crate::opcodes::op_30(self, bus, ctx),
            0x31 => crate::opcodes::op_31(self, bus, ctx),
            0x32 => crate::opcodes::op_32(self, bus, ctx),
            0x33 => crate::opcodes::op_33(self, bus, ctx),
            0x34 => crate::opcodes::op_34(self, bus, ctx),
            0x35 => crate::opcodes::op_35(self, bus, ctx),
            0x36 => crate::opcodes::op_36(self, bus, ctx),
            0x37 => crate::opcodes::op_37(self, bus, ctx),
            0x38 => crate::opcodes::op_38(self, bus, ctx),
            0x39 => crate::opcodes::op_39(self, bus, ctx),
            0x3a => crate::opcodes::op_3a(self, bus, ctx),
            0x3b => crate::opcodes::op_3b(self, bus, ctx),
            0x3c => crate::opcodes::op_3c(self, bus, ctx),
            0x3d => crate::opcodes::op_3d(self, bus, ctx),
            0x3e => crate::opcodes::op_3e(self, bus, ctx),
            0x3f => crate::opcodes::op_3f(self, bus, ctx),
            0x40 => crate::opcodes::op_40(self, bus, ctx),
            0x41 => crate::opcodes::op_41(self, bus, ctx),
            0x42 => crate::opcodes::op_42(self, bus, ctx),
            0x43 => crate::opcodes::op_43(self, bus, ctx),
            0x44 => crate::opcodes::op_44(self, bus, ctx),
            0x45 => crate::opcodes::op_45(self, bus, ctx),
            0x46 => crate::opcodes::op_46(self, bus, ctx),
            0x47 => crate::opcodes::op_47(self, bus, ctx),
            0x48 => crate::opcodes::op_48(self, bus, ctx),
            0x49 => crate::opcodes::op_49(self, bus, ctx),
            0x4a => crate::opcodes::op_4a(self, bus, ctx),
            0x4b => crate::opcodes::op_4b(self, bus, ctx),
            0x4c => crate::opcodes::op_4c(self, bus, ctx),
            0x4d => crate::opcodes::op_4d(self, bus, ctx),
            0x4e => crate::opcodes::op_4e(self, bus, ctx),
            0x4f => crate::opcodes::op_4f(self, bus, ctx),
            0x50 => crate::opcodes::op_50(self, bus, ctx),
            0x51 => crate::opcodes::op_51(self, bus, ctx),
            0x52 => crate::opcodes::op_52(self, bus, ctx),
            0x53 => crate::opcodes::op_53(self, bus, ctx),
            0x54 => crate::opcodes::op_54(self, bus, ctx),
            0x55 => crate::opcodes::op_55(self, bus, ctx),
            0x56 => crate::opcodes::op_56(self, bus, ctx),
            0x57 => crate::opcodes::op_57(self, bus, ctx),
            0x58 => crate::opcodes::op_58(self, bus, ctx),
            0x59 => crate::opcodes::op_59(self, bus, ctx),
            0x5a => crate::opcodes::op_5a(self, bus, ctx),
            0x5b => crate::opcodes::op_5b(self, bus, ctx),
            0x5c => crate::opcodes::op_5c(self, bus, ctx),
            0x5d => crate::opcodes::op_5d(self, bus, ctx),
            0x5e => crate::opcodes::op_5e(self, bus, ctx),
            0x5f => crate::opcodes::op_5f(self, bus, ctx),
            0x60 => crate::opcodes::op_60(self, bus, ctx),
            0x61 => crate::opcodes::op_61(self, bus, ctx),
            0x62 => crate::opcodes::op_62(self, bus, ctx),
            0x63 => crate::opcodes::op_63(self, bus, ctx),
            0x64 => crate::opcodes::op_64(self, bus, ctx),
            0x65 => crate::opcodes::op_65(self, bus, ctx),
            0x66 => crate::opcodes::op_66(self, bus, ctx),
            0x67 => crate::opcodes::op_67(self, bus, ctx),
            0x68 => crate::opcodes::op_68(self, bus, ctx),
            0x69 => crate::opcodes::op_69(self, bus, ctx),
            0x6a => crate::opcodes::op_6a(self, bus, ctx),
            0x6b => crate::opcodes::op_6b(self, bus, ctx),
            0x6c => crate::opcodes::op_6c(self, bus, ctx),
            0x6d => crate::opcodes::op_6d(self, bus, ctx),
            0x6e => crate::opcodes::op_6e(self, bus, ctx),
            0x6f => crate::opcodes::op_6f(self, bus, ctx),
            0x70 => crate::opcodes::op_70(self, bus, ctx),
            0x71 => crate::opcodes::op_71(self, bus, ctx),
            0x72 => crate::opcodes::op_72(self, bus, ctx),
            0x73 => crate::opcodes::op_73(self, bus, ctx),
            0x74 => crate::opcodes::op_74(self, bus, ctx),
            0x75 => crate::opcodes::op_75(self, bus, ctx),
            0x76 => crate::opcodes::op_76(self, bus, ctx),
            0x77 => crate::opcodes::op_77(self, bus, ctx),
            0x78 => crate::opcodes::op_78(self, bus, ctx),
            0x79 => crate::opcodes::op_79(self, bus, ctx),
            0x7a => crate::opcodes::op_7a(self, bus, ctx),
            0x7b => crate::opcodes::op_7b(self, bus, ctx),
            0x7c => crate::opcodes::op_7c(self, bus, ctx),
            0x7d => crate::opcodes::op_7d(self, bus, ctx),
            0x7e => crate::opcodes::op_7e(self, bus, ctx),
            0x7f => crate::opcodes::op_7f(self, bus, ctx),
            0x80 => crate::opcodes::op_80(self, bus, ctx),
            0x81 => crate::opcodes::op_81(self, bus, ctx),
            0x82 => crate::opcodes::op_82(self, bus, ctx),
            0x83 => crate::opcodes::op_83(self, bus, ctx),
            0x84 => crate::opcodes::op_84(self, bus, ctx),
            0x85 => crate::opcodes::op_85(self, bus, ctx),
            0x86 => crate::opcodes::op_86(self, bus, ctx),
            0x87 => crate::opcodes::op_87(self, bus, ctx),
            0x88 => crate::opcodes::op_88(self, bus, ctx),
            0x89 => crate::opcodes::op_89(self, bus, ctx),
            0x8a => crate::opcodes::op_8a(self, bus, ctx),
            0x8b => crate::opcodes::op_8b(self, bus, ctx),
            0x8c => crate::opcodes::op_8c(self, bus, ctx),
            0x8d => crate::opcodes::op_8d(self, bus, ctx),
            0x8e => crate::opcodes::op_8e(self, bus, ctx),
            0x8f => crate::opcodes::op_8f(self, bus, ctx),
            0x90 => crate::opcodes::op_90(self, bus, ctx),
            0x91 => crate::opcodes::op_91(self, bus, ctx),
            0x92 => crate::opcodes::op_92(self, bus, ctx),
            0x93 => crate::opcodes::op_93(self, bus, ctx),
            0x94 => crate::opcodes::op_94(self, bus, ctx),
            0x95 => crate::opcodes::op_95(self, bus, ctx),
            0x96 => crate::opcodes::op_96(self, bus, ctx),
            0x97 => crate::opcodes::op_97(self, bus, ctx),
            0x98 => crate::opcodes::op_98(self, bus, ctx),
            0x99 => crate::opcodes::op_99(self, bus, ctx),
            0x9a => crate::opcodes::op_9a(self, bus, ctx),
            0x9b => crate::opcodes::op_9b(self, bus, ctx),
            0x9c => crate::opcodes::op_9c(self, bus, ctx),
            0x9d => crate::opcodes::op_9d(self, bus, ctx),
            0x9e => crate::opcodes::op_9e(self, bus, ctx),
            0x9f => crate::opcodes::op_9f(self, bus, ctx),
            0xa0 => crate::opcodes::op_a0(self, bus, ctx),
            0xa1 => crate::opcodes::op_a1(self, bus, ctx),
            0xa2 => crate::opcodes::op_a2(self, bus, ctx),
            0xa3 => crate::opcodes::op_a3(self, bus, ctx),
            0xa4 => crate::opcodes::op_a4(self, bus, ctx),
            0xa5 => crate::opcodes::op_a5(self, bus, ctx),
            0xa6 => crate::opcodes::op_a6(self, bus, ctx),
            0xa7 => crate::opcodes::op_a7(self, bus, ctx),
            0xa8 => crate::opcodes::op_a8(self, bus, ctx),
            0xa9 => crate::opcodes::op_a9(self, bus, ctx),
            0xaa => crate::opcodes::op_aa(self, bus, ctx),
            0xab => crate::opcodes::op_ab(self, bus, ctx),
            0xac => crate::opcodes::op_ac(self, bus, ctx),
            0xad => crate::opcodes::op_ad(self, bus, ctx),
            0xae => crate::opcodes::op_ae(self, bus, ctx),
            0xaf => crate::opcodes::op_af(self, bus, ctx),
            0xb0 => crate::opcodes::op_b0(self, bus, ctx),
            0xb1 => crate::opcodes::op_b1(self, bus, ctx),
            0xb2 => crate::opcodes::op_b2(self, bus, ctx),
            0xb3 => crate::opcodes::op_b3(self, bus, ctx),
            0xb4 => crate::opcodes::op_b4(self, bus, ctx),
            0xb5 => crate::opcodes::op_b5(self, bus, ctx),
            0xb6 => crate::opcodes::op_b6(self, bus, ctx),
            0xb7 => crate::opcodes::op_b7(self, bus, ctx),
            0xb8 => crate::opcodes::op_b8(self, bus, ctx),
            0xb9 => crate::opcodes::op_b9(self, bus, ctx),
            0xba => crate::opcodes::op_ba(self, bus, ctx),
            0xbb => crate::opcodes::op_bb(self, bus, ctx),
            0xbc => crate::opcodes::op_bc(self, bus, ctx),
            0xbd => crate::opcodes::op_bd(self, bus, ctx),
            0xbe => crate::opcodes::op_be(self, bus, ctx),
            0xbf => crate::opcodes::op_bf(self, bus, ctx),
            0xc0 => crate::opcodes::op_c0(self, bus, ctx),
            0xc1 => crate::opcodes::op_c1(self, bus, ctx),
            0xc2 => crate::opcodes::op_c2(self, bus, ctx),
            0xc3 => crate::opcodes::op_c3(self, bus, ctx),
            0xc4 => crate::opcodes::op_c4(self, bus, ctx),
            0xc5 => crate::opcodes::op_c5(self, bus, ctx),
            0xc6 => crate::opcodes::op_c6(self, bus, ctx),
            0xc7 => crate::opcodes::op_c7(self, bus, ctx),
            0xc8 => crate::opcodes::op_c8(self, bus, ctx),
            0xc9 => crate::opcodes::op_c9(self, bus, ctx),
            0xca => crate::opcodes::op_ca(self, bus, ctx),
            0xcb => crate::opcodes::op_cb(self, bus, ctx),
            0xcc => crate::opcodes::op_cc(self, bus, ctx),
            0xcd => crate::opcodes::op_cd(self, bus, ctx),
            0xce => crate::opcodes::op_ce(self, bus, ctx),
            0xcf => crate::opcodes::op_cf(self, bus, ctx),
            0xd0 => crate::opcodes::op_d0(self, bus, ctx),
            0xd1 => crate::opcodes::op_d1(self, bus, ctx),
            0xd2 => crate::opcodes::op_d2(self, bus, ctx),
            0xd3 => crate::opcodes::op_d3(self, bus, ctx),
            0xd4 => crate::opcodes::op_d4(self, bus, ctx),
            0xd5 => crate::opcodes::op_d5(self, bus, ctx),
            0xd6 => crate::opcodes::op_d6(self, bus, ctx),
            0xd7 => crate::opcodes::op_d7(self, bus, ctx),
            0xd8 => crate::opcodes::op_d8(self, bus, ctx),
            0xd9 => crate::opcodes::op_d9(self, bus, ctx),
            0xda => crate::opcodes::op_da(self, bus, ctx),
            0xdb => crate::opcodes::op_db(self, bus, ctx),
            0xdc => crate::opcodes::op_dc(self, bus, ctx),
            0xdd => crate::opcodes::op_dd(self, bus, ctx),
            0xde => crate::opcodes::op_de(self, bus, ctx),
            0xdf => crate::opcodes::op_df(self, bus, ctx),
            0xe0 => crate::opcodes::op_e0(self, bus, ctx),
            0xe1 => crate::opcodes::op_e1(self, bus, ctx),
            0xe2 => crate::opcodes::op_e2(self, bus, ctx),
            0xe3 => crate::opcodes::op_e3(self, bus, ctx),
            0xe4 => crate::opcodes::op_e4(self, bus, ctx),
            0xe5 => crate::opcodes::op_e5(self, bus, ctx),
            0xe6 => crate::opcodes::op_e6(self, bus, ctx),
            0xe7 => crate::opcodes::op_e7(self, bus, ctx),
            0xe8 => crate::opcodes::op_e8(self, bus, ctx),
            0xe9 => crate::opcodes::op_e9(self, bus, ctx),
            0xea => crate::opcodes::op_ea(self, bus, ctx),
            0xeb => crate::opcodes::op_eb(self, bus, ctx),
            0xec => crate::opcodes::op_ec(self, bus, ctx),
            0xed => crate::opcodes::op_ed(self, bus, ctx),
            0xee => crate::opcodes::op_ee(self, bus, ctx),
            0xef => crate::opcodes::op_ef(self, bus, ctx),
            0xf0 => crate::opcodes::op_f0(self, bus, ctx),
            0xf1 => crate::opcodes::op_f1(self, bus, ctx),
            0xf2 => crate::opcodes::op_f2(self, bus, ctx),
            0xf3 => crate::opcodes::op_f3(self, bus, ctx),
            0xf4 => crate::opcodes::op_f4(self, bus, ctx),
            0xf5 => crate::opcodes::op_f5(self, bus, ctx),
            0xf6 => crate::opcodes::op_f6(self, bus, ctx),
            0xf7 => crate::opcodes::op_f7(self, bus, ctx),
            0xf8 => crate::opcodes::op_f8(self, bus, ctx),
            0xf9 => crate::opcodes::op_f9(self, bus, ctx),
            0xfa => crate::opcodes::op_fa(self, bus, ctx),
            0xfb => crate::opcodes::op_fb(self, bus, ctx),
            0xfc => crate::opcodes::op_fc(self, bus, ctx),
            0xfd => crate::opcodes::op_fd(self, bus, ctx),
            0xfe => crate::opcodes::op_fe(self, bus, ctx),
            0xff => crate::opcodes::op_ff(self, bus, ctx),
        };
        self.cycles = self.cycles.wrapping_add(elapsed);
        elapsed
    }
}
