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

const FLAG_C: u8 = 0x01;
const FLAG_Z: u8 = 0x02;
const FLAG_I: u8 = 0x04;
const FLAG_D: u8 = 0x08;
const FLAG_B: u8 = 0x10;
const FLAG_U: u8 = 0x20;
const FLAG_V: u8 = 0x40;
const FLAG_N: u8 = 0x80;

impl Default for CPU {
    fn default() -> Self {
        Self::new()
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
            pending_interrupt: Some(Interrupt::Reset),
            pending_so: false,
            last_so_pin: false,
            cycles: 0,
        }
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    pub fn reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        self.pending_interrupt = Some(Interrupt::Reset);
        self.step(bus, ctx);
    }
    
    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    pub fn set_so_pin(&mut self, active: bool) {
        if active && !self.last_so_pin {
            self.pending_so = true;
        }
        self.last_so_pin = active;
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u8 {
        let val = bus.read(addr, self.cycles, ctx);
        self.cycles = self.cycles.wrapping_add(1);
        val
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn write<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16, val: u8) {
        bus.write(addr, val, self.cycles, ctx);
        self.cycles = self.cycles.wrapping_add(1);
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn read_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        let val = bus.read(self.pc, self.cycles, ctx);
        self.cycles = self.cycles.wrapping_add(1);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn read_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u16 {
        let lo = bus.read(addr, self.cycles, ctx) as u16;
        self.cycles = self.cycles.wrapping_add(1);
        let hi = bus.read(addr.wrapping_add(1), self.cycles, ctx) as u16;
        self.cycles = self.cycles.wrapping_add(1);
        (hi << 8) | lo
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn read_word_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.read_pc(bus, ctx) as u16;
        let hi = self.read_pc(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn push<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u8) {
        self.write(bus, ctx, 0x0100 | (self.s as u16), val);
        self.s = self.s.wrapping_sub(1);
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn pop<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.read(bus, ctx, 0x0100 | (self.s as u16))
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn push_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u16) {
        self.push(bus, ctx, (val >> 8) as u8);
        self.push(bus, ctx, val as u8);
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn pop_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.pop(bus, ctx) as u16;
        let hi = self.pop(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn set_z(&mut self, val: u8) {
        if val == 0 { self.p |= FLAG_Z; } else { self.p &= !FLAG_Z; }
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn set_n(&mut self, val: u8) {
        if val & 0x80 != 0 { self.p |= FLAG_N; } else { self.p &= !FLAG_N; }
    }

    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[inline(always)]
    fn set_zn(&mut self, val: u8) {
        self.set_z(val);
        self.set_n(val);
    }
    
    #[unsafe(link_section = ".data.pico1541_emulator_core")]
    #[allow(unused_variables, unused_assignments)]
    pub fn step<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        if let Some(interrupt) = self.pending_interrupt.take() {
            match interrupt {
                Interrupt::Reset => {
                    self.pc = self.read_word(bus, ctx, 0xFFFC);
                    self.s = self.s.wrapping_sub(3);
                    self.p |= FLAG_I;
                    return 7;
                }
                Interrupt::Nmi => {
                    self.push_word(bus, ctx, self.pc);
                    self.push(bus, ctx, self.p | FLAG_U);
                    self.p |= FLAG_I;
                    self.pc = self.read_word(bus, ctx, 0xFFFA);
                    return 7;
                }
                Interrupt::Irq => {
                    if self.p & FLAG_I == 0 {
                        self.push_word(bus, ctx, self.pc);
                        self.push(bus, ctx, self.p | FLAG_U);
                        self.p |= FLAG_I;
                        self.pc = self.read_word(bus, ctx, 0xFFFE);
                        return 7;
                    }
                }
            }
        }
        
        // Handle IRQ checking inside step to be close to what mos6502 did
        if bus.nmi_pending() {
            self.pending_interrupt = Some(Interrupt::Nmi);
        } else if bus.irq_pending() {
            self.pending_interrupt = Some(Interrupt::Irq);
        }

        if self.pending_so {
            self.p |= FLAG_V;
            self.pending_so = false;
        }

        let opcode = self.read_pc(bus, ctx);
        let mut cycles = 0;
        let mut page_cross = false;

        match opcode {

            0x00 => {
                let addr = 0;
                self.pc = self.pc.wrapping_add(1);
                self.push_word(bus, ctx, self.pc);
                self.push(bus, ctx, self.p | FLAG_B | FLAG_U);
                self.p |= FLAG_I;
                self.pc = self.read_word(bus, ctx, 0xFFFE);
                cycles += 7;
                cycles
            }
            0x01 => {
                let addr = { 
                    let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); 
                    let lo = self.read(bus, ctx, ptr as u16) as u16; 
                    let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo 
                };
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 6;
                cycles
            }
            0x02 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x03 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x04 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x05 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 3;
                cycles
            }
            0x06 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val <<= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0x07 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x08 => {
                let addr = 0;
                self.push(bus, ctx, self.p | FLAG_B | FLAG_U);
                cycles += 3;
                cycles
            }
            0x09 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.a |= val; self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x0A => {
                let addr = 0;
                if self.a & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.a <<= 1;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x0B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x0C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x0D => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x0E => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val <<= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x0F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x10 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_N == 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0x11 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0x12 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x13 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x14 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x15 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x16 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val <<= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x17 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x18 => {
                let addr = 0;
                self.p &= !FLAG_C;
                cycles += 2;
                cycles
            }
            0x19 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x1A => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x1B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x1C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x1D => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a |= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x1E => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val <<= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0x1F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x20 => {
                let addr = self.read_word_pc(bus, ctx);
                self.push_word(bus, ctx, self.pc.wrapping_sub(1));
                self.pc = addr;
                cycles += 6;
                cycles
            }
            0x21 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 6;
                cycles
            }
            0x22 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x23 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x24 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                let res = self.a & val;
                self.set_z(res);
                self.p = (self.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
                cycles += 3;
                cycles
            }
            0x25 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 3;
                cycles
            }
            0x26 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val << 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0x27 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x28 => {
                let addr = 0;
                self.p = self.pop(bus, ctx) | FLAG_U;
                cycles += 4;
                cycles
            }
            0x29 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.a &= val; self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x2A => {
                let addr = 0;
                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };
                if self.a & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.a = (self.a << 1) | c;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x2B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x2C => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                let res = self.a & val;
                self.set_z(res);
                self.p = (self.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
                cycles += 4;
                cycles
            }
            0x2D => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x2E => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val << 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x2F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x30 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_N != 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0x31 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0x32 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x33 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x34 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x35 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x36 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val << 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x37 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x38 => {
                let addr = 0;
                self.p |= FLAG_C;
                cycles += 2;
                cycles
            }
            0x39 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x3A => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x3B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x3C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x3D => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a &= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x3E => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val << 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0x3F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x40 => {
                let addr = 0;
                self.p = self.pop(bus, ctx) | FLAG_U;
                self.pc = self.pop_word(bus, ctx);
                cycles += 6;
                cycles
            }
            0x41 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 6;
                cycles
            }
            0x42 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x43 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x44 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x45 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 3;
                cycles
            }
            0x46 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val >>= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0x47 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x48 => {
                let addr = 0;
                self.push(bus, ctx, self.a);
                cycles += 3;
                cycles
            }
            0x49 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.a ^= val; self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x4A => {
                let addr = 0;
                if self.a & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.a >>= 1;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x4B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x4C => {
                let addr = self.read_word_pc(bus, ctx);
                self.pc = addr;
                cycles += 3;
                cycles
            }
            0x4D => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x4E => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val >>= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x4F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x50 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_V == 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0x51 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0x52 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x53 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x54 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x55 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x56 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val >>= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x57 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x58 => {
                let addr = 0;
                self.p &= !FLAG_I;
                cycles += 2;
                cycles
            }
            0x59 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x5A => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x5B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x5C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x5D => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a ^= val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x5E => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val >>= 1;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0x5F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x60 => {
                let addr = 0;
                self.pc = self.pop_word(bus, ctx).wrapping_add(1);
                cycles += 6;
                cycles
            }
            0x61 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
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
                cycles += 6;
                cycles
            }
            0x62 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x63 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x64 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x65 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
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
                cycles += 3;
                cycles
            }
            0x66 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val >> 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0x67 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x68 => {
                let addr = 0;
                self.a = self.pop(bus, ctx);
                self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0x69 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
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
                cycles += 2;
                cycles
            }
            0x6A => {
                let addr = 0;
                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };
                if self.a & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.a = (self.a >> 1) | c;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x6B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x6C => {
                let addr = { let ptr = self.read_word_pc(bus, ctx); if ptr & 0x00FF == 0x00FF { (self.read(bus, ctx, ptr & 0xFF00) as u16) << 8 | self.read(bus, ctx, ptr) as u16 } else { self.read_word(bus, ctx, ptr) } };
                self.pc = addr;
                cycles += 5;
                cycles
            }
            0x6D => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                cycles
            }
            0x6E => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val >> 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x6F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x70 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_V != 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0x71 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0x72 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x73 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x74 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x75 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                cycles
            }
            0x76 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val >> 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0x77 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x78 => {
                let addr = 0;
                self.p |= FLAG_I;
                cycles += 2;
                cycles
            }
            0x79 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x7A => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x7B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x7C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x7D => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0x7E => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                val = (val >> 1) | c;
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0x7F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x80 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x81 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                self.write(bus, ctx, addr, self.a);
                cycles += 6;
                cycles
            }
            0x82 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x83 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x84 => {
                let addr = self.read_pc(bus, ctx) as u16;
                self.write(bus, ctx, addr, self.y);
                cycles += 3;
                cycles
            }
            0x85 => {
                let addr = self.read_pc(bus, ctx) as u16;
                self.write(bus, ctx, addr, self.a);
                cycles += 3;
                cycles
            }
            0x86 => {
                let addr = self.read_pc(bus, ctx) as u16;
                self.write(bus, ctx, addr, self.x);
                cycles += 3;
                cycles
            }
            0x87 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x88 => {
                let addr = 0;
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
                cycles += 2;
                cycles
            }
            0x89 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x8A => {
                let addr = 0;
                self.a = self.x;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x8B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x8C => {
                let addr = self.read_word_pc(bus, ctx);
                self.write(bus, ctx, addr, self.y);
                cycles += 4;
                cycles
            }
            0x8D => {
                let addr = self.read_word_pc(bus, ctx);
                self.write(bus, ctx, addr, self.a);
                cycles += 4;
                cycles
            }
            0x8E => {
                let addr = self.read_word_pc(bus, ctx);
                self.write(bus, ctx, addr, self.x);
                cycles += 4;
                cycles
            }
            0x8F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x90 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_C == 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0x91 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                self.write(bus, ctx, addr, self.a);
                cycles += 6;
                cycles
            }
            0x92 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x93 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x94 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                self.write(bus, ctx, addr, self.y);
                cycles += 4;
                cycles
            }
            0x95 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                self.write(bus, ctx, addr, self.a);
                cycles += 4;
                cycles
            }
            0x96 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.y) as u16;
                self.write(bus, ctx, addr, self.x);
                cycles += 4;
                cycles
            }
            0x97 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x98 => {
                let addr = 0;
                self.a = self.y;
                self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0x99 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                self.write(bus, ctx, addr, self.a);
                cycles += 5;
                cycles
            }
            0x9A => {
                let addr = 0;
                self.s = self.x;
                cycles += 2;
                cycles
            }
            0x9B => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x9C => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x9D => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                self.write(bus, ctx, addr, self.a);
                cycles += 5;
                cycles
            }
            0x9E => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0x9F => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xA0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.y = val; self.set_zn(self.y);
                cycles += 2;
                cycles
            }
            0xA1 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 6;
                cycles
            }
            0xA2 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.x = val; self.set_zn(self.x);
                cycles += 2;
                cycles
            }
            0xA3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xA4 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.y = val; self.set_zn(self.y);
                cycles += 3;
                cycles
            }
            0xA5 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 3;
                cycles
            }
            0xA6 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                self.x = val; self.set_zn(self.x);
                cycles += 3;
                cycles
            }
            0xA7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xA8 => {
                let addr = 0;
                self.y = self.a;
                self.set_zn(self.y);
                cycles += 2;
                cycles
            }
            0xA9 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                self.a = val; self.set_zn(self.a);
                cycles += 2;
                cycles
            }
            0xAA => {
                let addr = 0;
                self.x = self.a;
                self.set_zn(self.x);
                cycles += 2;
                cycles
            }
            0xAB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xAC => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.y = val; self.set_zn(self.y);
                cycles += 4;
                cycles
            }
            0xAD => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0xAE => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                self.x = val; self.set_zn(self.x);
                cycles += 4;
                cycles
            }
            0xAF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xB0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_C != 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0xB1 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0xB2 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xB3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xB4 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                self.y = val; self.set_zn(self.y);
                cycles += 4;
                cycles
            }
            0xB5 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 4;
                cycles
            }
            0xB6 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.y) as u16;
                let val = self.read(bus, ctx, addr);
                self.x = val; self.set_zn(self.x);
                cycles += 4;
                cycles
            }
            0xB7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xB8 => {
                let addr = 0;
                self.p &= !FLAG_V;
                cycles += 2;
                cycles
            }
            0xB9 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xBA => {
                let addr = 0;
                self.x = self.s;
                self.set_zn(self.x);
                cycles += 2;
                cycles
            }
            0xBB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xBC => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.y = val; self.set_zn(self.y);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xBD => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.a = val; self.set_zn(self.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xBE => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                self.x = val; self.set_zn(self.x);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xBF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xC0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                let diff = (self.y as u16).wrapping_sub(val as u16);
                if self.y >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 2;
                cycles
            }
            0xC1 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 6;
                cycles
            }
            0xC2 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xC3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xC4 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                let diff = (self.y as u16).wrapping_sub(val as u16);
                if self.y >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 3;
                cycles
            }
            0xC5 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 3;
                cycles
            }
            0xC6 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_sub(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0xC7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xC8 => {
                let addr = 0;
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
                cycles += 2;
                cycles
            }
            0xC9 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 2;
                cycles
            }
            0xCA => {
                let addr = 0;
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
                cycles += 2;
                cycles
            }
            0xCB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xCC => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                let diff = (self.y as u16).wrapping_sub(val as u16);
                if self.y >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                cycles
            }
            0xCD => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                cycles
            }
            0xCE => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_sub(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0xCF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xD0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_Z == 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0xD1 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0xD2 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xD3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xD4 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xD5 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                cycles
            }
            0xD6 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_sub(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0xD7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xD8 => {
                let addr = 0;
                self.p &= !FLAG_D;
                cycles += 2;
                cycles
            }
            0xD9 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xDA => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xDB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xDC => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xDD => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
                let diff = (self.a as u16).wrapping_sub(val as u16);
                if self.a >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xDE => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_sub(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0xDF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xE0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
                let diff = (self.x as u16).wrapping_sub(val as u16);
                if self.x >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 2;
                cycles
            }
            0xE1 => {
                let addr = { let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = self.read(bus, ctx, addr);
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
                cycles += 6;
                cycles
            }
            0xE2 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xE3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xE4 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
                let diff = (self.x as u16).wrapping_sub(val as u16);
                if self.x >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 3;
                cycles
            }
            0xE5 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = self.read(bus, ctx, addr);
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
                cycles += 3;
                cycles
            }
            0xE6 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_add(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 5;
                cycles
            }
            0xE7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xE8 => {
                let addr = 0;
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
                cycles += 2;
                cycles
            }
            0xE9 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let val = addr as u8;
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
                cycles += 2;
                cycles
            }
            0xEA => {
                let addr = 0;
                cycles += 2;
                cycles
            }
            0xEB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xEC => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
                let diff = (self.x as u16).wrapping_sub(val as u16);
                if self.x >= val { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }
                self.set_zn(diff as u8);
                cycles += 4;
                cycles
            }
            0xED => {
                let addr = self.read_word_pc(bus, ctx);
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                cycles
            }
            0xEE => {
                let addr = self.read_word_pc(bus, ctx);
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_add(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0xEF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xF0 => {
                let addr = self.read_pc(bus, ctx) as u16;
                let rel = addr as i8;
                if self.p & FLAG_Z != 0 {
                    let new_pc = self.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };
                    self.pc = new_pc;
                }
                cycles += 2;
                cycles
            }
            0xF1 => {
                let addr = { let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }
            0xF2 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xF3 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xF4 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xF5 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                cycles
            }
            0xF6 => {
                let addr = self.read_pc(bus, ctx).wrapping_add(self.x) as u16;
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_add(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 6;
                cycles
            }
            0xF7 => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xF8 => {
                let addr = 0;
                self.p |= FLAG_D;
                cycles += 2;
                cycles
            }
            0xF9 => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xFA => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xFB => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xFC => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
            0xFD => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = self.read(bus, ctx, addr);
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
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }
            0xFE => {
                let addr = { let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = self.read(bus, ctx, addr);
                val = val.wrapping_add(1);
                self.write(bus, ctx, addr, val);
                self.set_zn(val);
                cycles += 7;
                cycles
            }
            0xFF => { self.read_pc(bus, ctx); 2 } // Undocumented NOP/Ignored
        }
    }
}
