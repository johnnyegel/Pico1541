use crate::bus::Bus;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interrupt {
    Reset,
    Nmi,
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
    #[inline]
    pub fn read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32, addr: u16) -> u8 {
        let val = bus.read(addr, self.cycles.wrapping_add(*cycle_offset), ctx);
        *cycle_offset += 1;
        val
    }

    #[inline]
    pub fn write<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32, addr: u16, val: u8) {
        bus.write(addr, val, self.cycles.wrapping_add(*cycle_offset), ctx);
        *cycle_offset += 1;
    }

    #[inline]
    pub fn read_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32) -> u8 {
        let val = bus.read(self.pc, self.cycles.wrapping_add(*cycle_offset), ctx);
        *cycle_offset += 1;
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline]
    pub fn read_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32, addr: u16) -> u16 {
        let lo = self.read(bus, ctx, cycle_offset, addr) as u16;
        let hi = self.read(bus, ctx, cycle_offset, addr.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn read_word_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32) -> u16 {
        let lo = self.read_pc(bus, ctx, cycle_offset) as u16;
        let hi = self.read_pc(bus, ctx, cycle_offset) as u16;
        (hi << 8) | lo
    }

    #[inline]
    pub fn push<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32, val: u8) {
        self.write(bus, ctx, cycle_offset, 0x0100 | (self.s as u16), val);
        self.s = self.s.wrapping_sub(1);
    }

    #[inline]
    pub fn pop<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.read(bus, ctx, cycle_offset, 0x0100 | (self.s as u16))
    }

    #[inline]
    pub fn push_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32, val: u16) {
        self.push(bus, ctx, cycle_offset, (val >> 8) as u8);
        self.push(bus, ctx, cycle_offset, val as u8);
    }

    #[inline]
    pub fn pop_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, cycle_offset: &mut u32) -> u16 {
        let lo = self.pop(bus, ctx, cycle_offset) as u16;
        let hi = self.pop(bus, ctx, cycle_offset) as u16;
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
        }
    }

    pub fn reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        let mut offset = 0;
        self.pc = self.read_word(bus, ctx, &mut offset, 0xFFFC);
        self.s = self.s.wrapping_sub(3);
        self.p |= FLAG_I;
    }
}

impl CPU {
    pub fn set_so_pin(&mut self, active: bool) {
        if active && !self.last_so_pin {
            self.pending_so = true;
        }
        self.last_so_pin = active;
    }

    #[inline(always)]
    pub fn do_adc(&mut self, val: u8) {
        if self.p & FLAG_D != 0 {
            let a = self.a; let b = val; let c = self.p & FLAG_C;
            let mut low = (a & 0x0F) + (b & 0x0F) + c;
            if low > 9 { low += 6; }
            let mut high = (a >> 4) + (b >> 4) + (if low > 15 { 1 } else { 0 });
            let al = a as u16; let bl = b as u16; let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 { self.p |= FLAG_V; }
            if (res_bin & 0x80) != 0 { self.p |= FLAG_N; }
            if (res_bin as u8) == 0 { self.p |= FLAG_Z; }
            if high > 9 { high += 6; }
            if high > 15 { self.p |= FLAG_C; }
            self.a = (low & 0x0F) | ((high & 0x0F) << 4);
        } else {
            let a = self.a as u16; let b = val as u16; let c = (self.p & FLAG_C) as u16;
            let res = a + b + c;
            self.p &= !0xC3;
            if res > 0xFF { self.p |= FLAG_C; }
            let r = res as u8;
            if r == 0 { self.p |= FLAG_Z; }
            if (r & 0x80) != 0 { self.p |= FLAG_N; }
            if ((a ^ res) & (b ^ res) & 0x80) != 0 { self.p |= FLAG_V; }
            self.a = r;
        }
    }

    #[inline(always)]
    pub fn do_sbc(&mut self, val: u8) {
        if self.p & FLAG_D != 0 {
            let a = self.a; let b = val; let c = self.p & FLAG_C;
            let mut low = (a & 0x0F) as i32 - (b & 0x0F) as i32 - (1 - c as i32);
            let mut high = (a >> 4) as i32 - (b >> 4) as i32 - (if low < 0 { 1 } else { 0 });
            let al = a as u16; let bl = (!b) as u16; let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if res_bin > 0xFF { self.p |= FLAG_C; }
            if (res_bin as u8) == 0 { self.p |= FLAG_Z; }
            if (res_bin & 0x80) != 0 { self.p |= FLAG_N; }
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 { self.p |= FLAG_V; }
            if low < 0 { low -= 6; }
            if high < 0 { high -= 6; }
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
        } else {
            let a = self.a as u16; let b = (!val) as u16; let c = (self.p & FLAG_C) as u16;
            let res = a + b + c;
            self.p &= !0xC3;
            if res > 0xFF { self.p |= FLAG_C; }
            let r = res as u8;
            if r == 0 { self.p |= FLAG_Z; }
            if (r & 0x80) != 0 { self.p |= FLAG_N; }
            if ((a ^ res) & (b ^ res) & 0x80) != 0 { self.p |= FLAG_V; }
            self.a = r;
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(never)]
    pub fn step<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, jump_table: &[crate::opcodes::OpcodeFn<B>; 256]) -> u32 {
        if let Some(interrupt) = self.pending_interrupt.take() {
            let mut offset = 0;
            match interrupt {
                Interrupt::Reset => {
                    self.pc = self.read_word(bus, ctx, &mut offset, 0xFFFC);
                    self.s = self.s.wrapping_sub(3);
                    self.p |= FLAG_I;
                    let elapsed = 7;
                    self.cycles = self.cycles.wrapping_add(elapsed);
                    return elapsed;
                }
                Interrupt::Nmi => {
                    self.push_word(bus, ctx, &mut offset, self.pc);
                    self.push(bus, ctx, &mut offset, self.p | FLAG_U);
                    self.p |= FLAG_I;
                    self.pc = self.read_word(bus, ctx, &mut offset, 0xFFFA);
                    let elapsed = 7;
                    self.cycles = self.cycles.wrapping_add(elapsed);
                    return elapsed;
                }
                Interrupt::Irq => {
                    if self.p & FLAG_I == 0 {
                        self.push_word(bus, ctx, &mut offset, self.pc);
                        self.push(bus, ctx, &mut offset, self.p | FLAG_U);
                        self.p |= FLAG_I;
                        self.pc = self.read_word(bus, ctx, &mut offset, 0xFFFE);
                        let elapsed = 7;
                        self.cycles = self.cycles.wrapping_add(elapsed);
                        return elapsed;
                    }
                }
            }
        }
        
        if bus.nmi_pending() {
            self.pending_interrupt = Some(Interrupt::Nmi);
        } else if bus.irq_pending() {
            self.pending_interrupt = Some(Interrupt::Irq);
        }

        if self.pending_so {
            self.p |= FLAG_V;
            self.pending_so = false;
        }

        let opcode = bus.read(self.pc, self.cycles, ctx);
        self.pc = self.pc.wrapping_add(1);
        
        let elapsed = jump_table[opcode as usize](self, bus, ctx);
        self.cycles = self.cycles.wrapping_add(elapsed);
        elapsed
    }
}
