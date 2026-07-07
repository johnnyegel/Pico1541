use crate::bus::Bus;

pub struct CPU {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    pub cycles: u32,
}


pub const FLAG_C: u8 = 0x01;
pub const FLAG_Z: u8 = 0x02;
pub const FLAG_I: u8 = 0x04;
pub const FLAG_D: u8 = 0x08;
pub const FLAG_B: u8 = 0x10;
pub const FLAG_U: u8 = 0x20;
pub const FLAG_V: u8 = 0x40;
pub const FLAG_N: u8 = 0x80;

impl CPU {
    #[inline]
    pub fn read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u8 {
        bus.read(addr, ctx)
    }

    #[inline]
    pub fn write<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16, val: u8) {
        bus.write(addr, val, ctx);
    }

    #[inline]
    pub fn read_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        let val = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    pub fn read_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u16 {
        let lo = bus.read(addr, ctx) as u16;
        let hi = bus.read(addr.wrapping_add(1), ctx) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn read_word_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.read_pc(bus, ctx) as u16;
        let hi = self.read_pc(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn push<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u8) {
        self.write(bus, ctx, 0x0100 | (self.s as u16), val);
        self.s = self.s.wrapping_sub(1);
    }

    #[inline]
    pub fn pop<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.read(bus, ctx, 0x0100 | (self.s as u16))
    }

    #[inline]
    pub fn push_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u16) {
        self.push(bus, ctx, (val >> 8) as u8);
        self.push(bus, ctx, val as u8);
    }

    #[inline]
    pub fn pop_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.pop(bus, ctx) as u16;
        let hi = self.pop(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn set_z(&mut self, val: u8) {
        if val == 0 { self.p |= FLAG_Z; } else { self.p &= !FLAG_Z; }
    }

    #[inline]
    pub fn set_n(&mut self, val: u8) {
        if val & 0x80 != 0 { self.p |= FLAG_N; } else { self.p &= !FLAG_N; }
    }

    #[inline]
    pub fn set_zn(&mut self, val: u8) {
        self.set_z(val);
        self.set_n(val);
    }
}

impl CPU {
    pub fn new() -> Self {
        Self {
            a: 0, x: 0, y: 0, s: 0xFD, pc: 0, p: 0x24, cycles: 0,
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(never)]
    pub fn step<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, jump_table: &[crate::opcodes::OpcodeFn<B>; 256]) -> u32 {
        let opcode = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);
        
        // Jump table dispatch
        let elapsed = jump_table[opcode as usize](self, bus, ctx);
        self.cycles = self.cycles.wrapping_add(elapsed);
        elapsed
    }
}

impl Default for CPU {
    fn default() -> Self {
        Self::new()
    }
}

impl CPU {
    pub fn reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        self.pc = self.read_word(bus, ctx, 0xFFFC);
        self.s = self.s.wrapping_sub(3);
        self.p |= FLAG_I;
    }
}
