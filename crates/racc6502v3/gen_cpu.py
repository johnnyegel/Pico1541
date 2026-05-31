import json

opcodes = {}

# Standard 6502 instructions
# Format: "OPC": [ (opcode, "mode", cycles, "page_cycles") ]

instructions = {
    "ADC": [ (0x69, "IMM", 2, 0), (0x65, "ZP", 3, 0), (0x75, "ZPX", 4, 0), (0x6D, "ABS", 4, 0), (0x7D, "ABSX", 4, 1), (0x79, "ABSY", 4, 1), (0x61, "INDX", 6, 0), (0x71, "INDY", 5, 1) ],
    "AND": [ (0x29, "IMM", 2, 0), (0x25, "ZP", 3, 0), (0x35, "ZPX", 4, 0), (0x2D, "ABS", 4, 0), (0x3D, "ABSX", 4, 1), (0x39, "ABSY", 4, 1), (0x21, "INDX", 6, 0), (0x31, "INDY", 5, 1) ],
    "ASL": [ (0x0A, "ACC", 2, 0), (0x06, "ZP", 5, 0), (0x16, "ZPX", 6, 0), (0x0E, "ABS", 6, 0), (0x1E, "ABSX", 7, 0) ],
    "BCC": [ (0x90, "REL", 2, 1) ],
    "BCS": [ (0xB0, "REL", 2, 1) ],
    "BEQ": [ (0xF0, "REL", 2, 1) ],
    "BIT": [ (0x24, "ZP", 3, 0), (0x2C, "ABS", 4, 0) ],
    "BMI": [ (0x30, "REL", 2, 1) ],
    "BNE": [ (0xD0, "REL", 2, 1) ],
    "BPL": [ (0x10, "REL", 2, 1) ],
    "BRK": [ (0x00, "IMP", 7, 0) ],
    "BVC": [ (0x50, "REL", 2, 1) ],
    "BVS": [ (0x70, "REL", 2, 1) ],
    "CLC": [ (0x18, "IMP", 2, 0) ],
    "CLD": [ (0xD8, "IMP", 2, 0) ],
    "CLI": [ (0x58, "IMP", 2, 0) ],
    "CLV": [ (0xB8, "IMP", 2, 0) ],
    "CMP": [ (0xC9, "IMM", 2, 0), (0xC5, "ZP", 3, 0), (0xD5, "ZPX", 4, 0), (0xCD, "ABS", 4, 0), (0xDD, "ABSX", 4, 1), (0xD9, "ABSY", 4, 1), (0xC1, "INDX", 6, 0), (0xD1, "INDY", 5, 1) ],
    "CPX": [ (0xE0, "IMM", 2, 0), (0xE4, "ZP", 3, 0), (0xEC, "ABS", 4, 0) ],
    "CPY": [ (0xC0, "IMM", 2, 0), (0xC4, "ZP", 3, 0), (0xCC, "ABS", 4, 0) ],
    "DEC": [ (0xC6, "ZP", 5, 0), (0xD6, "ZPX", 6, 0), (0xCE, "ABS", 6, 0), (0xDE, "ABSX", 7, 0) ],
    "DEX": [ (0xCA, "IMP", 2, 0) ],
    "DEY": [ (0x88, "IMP", 2, 0) ],
    "EOR": [ (0x49, "IMM", 2, 0), (0x45, "ZP", 3, 0), (0x55, "ZPX", 4, 0), (0x4D, "ABS", 4, 0), (0x5D, "ABSX", 4, 1), (0x59, "ABSY", 4, 1), (0x41, "INDX", 6, 0), (0x51, "INDY", 5, 1) ],
    "INC": [ (0xE6, "ZP", 5, 0), (0xF6, "ZPX", 6, 0), (0xEE, "ABS", 6, 0), (0xFE, "ABSX", 7, 0) ],
    "INX": [ (0xE8, "IMP", 2, 0) ],
    "INY": [ (0xC8, "IMP", 2, 0) ],
    "JMP": [ (0x4C, "ABS", 3, 0), (0x6C, "IND", 5, 0) ],
    "JSR": [ (0x20, "ABS", 6, 0) ],
    "LDA": [ (0xA9, "IMM", 2, 0), (0xA5, "ZP", 3, 0), (0xB5, "ZPX", 4, 0), (0xAD, "ABS", 4, 0), (0xBD, "ABSX", 4, 1), (0xB9, "ABSY", 4, 1), (0xA1, "INDX", 6, 0), (0xB1, "INDY", 5, 1) ],
    "LDX": [ (0xA2, "IMM", 2, 0), (0xA6, "ZP", 3, 0), (0xB6, "ZPY", 4, 0), (0xAE, "ABS", 4, 0), (0xBE, "ABSY", 4, 1) ],
    "LDY": [ (0xA0, "IMM", 2, 0), (0xA4, "ZP", 3, 0), (0xB4, "ZPX", 4, 0), (0xAC, "ABS", 4, 0), (0xBC, "ABSX", 4, 1) ],
    "LSR": [ (0x4A, "ACC", 2, 0), (0x46, "ZP", 5, 0), (0x56, "ZPX", 6, 0), (0x4E, "ABS", 6, 0), (0x5E, "ABSX", 7, 0) ],
    "NOP": [ (0xEA, "IMP", 2, 0) ],
    "ORA": [ (0x09, "IMM", 2, 0), (0x05, "ZP", 3, 0), (0x15, "ZPX", 4, 0), (0x0D, "ABS", 4, 0), (0x1D, "ABSX", 4, 1), (0x19, "ABSY", 4, 1), (0x01, "INDX", 6, 0), (0x11, "INDY", 5, 1) ],
    "PHA": [ (0x48, "IMP", 3, 0) ],
    "PHP": [ (0x08, "IMP", 3, 0) ],
    "PLA": [ (0x68, "IMP", 4, 0) ],
    "PLP": [ (0x28, "IMP", 4, 0) ],
    "ROL": [ (0x2A, "ACC", 2, 0), (0x26, "ZP", 5, 0), (0x36, "ZPX", 6, 0), (0x2E, "ABS", 6, 0), (0x3E, "ABSX", 7, 0) ],
    "ROR": [ (0x6A, "ACC", 2, 0), (0x66, "ZP", 5, 0), (0x76, "ZPX", 6, 0), (0x6E, "ABS", 6, 0), (0x7E, "ABSX", 7, 0) ],
    "RTI": [ (0x40, "IMP", 6, 0) ],
    "RTS": [ (0x60, "IMP", 6, 0) ],
    "SBC": [ (0xE9, "IMM", 2, 0), (0xE5, "ZP", 3, 0), (0xF5, "ZPX", 4, 0), (0xED, "ABS", 4, 0), (0xFD, "ABSX", 4, 1), (0xF9, "ABSY", 4, 1), (0xE1, "INDX", 6, 0), (0xF1, "INDY", 5, 1) ],
    "SEC": [ (0x38, "IMP", 2, 0) ],
    "SED": [ (0xF8, "IMP", 2, 0) ],
    "SEI": [ (0x78, "IMP", 2, 0) ],
    "STA": [ (0x85, "ZP", 3, 0), (0x95, "ZPX", 4, 0), (0x8D, "ABS", 4, 0), (0x9D, "ABSX", 5, 0), (0x99, "ABSY", 5, 0), (0x81, "INDX", 6, 0), (0x91, "INDY", 6, 0) ],
    "STX": [ (0x86, "ZP", 3, 0), (0x96, "ZPY", 4, 0), (0x8E, "ABS", 4, 0) ],
    "STY": [ (0x84, "ZP", 3, 0), (0x94, "ZPX", 4, 0), (0x8C, "ABS", 4, 0) ],
    "TAX": [ (0xAA, "IMP", 2, 0) ],
    "TAY": [ (0xA8, "IMP", 2, 0) ],
    "TSX": [ (0xBA, "IMP", 2, 0) ],
    "TXA": [ (0x8A, "IMP", 2, 0) ],
    "TXS": [ (0x9A, "IMP", 2, 0) ],
    "TYA": [ (0x98, "IMP", 2, 0) ]
}

modes_fetch = {
    "IMP": ("0", "0", "false"),
    "ACC": ("0", "0", "false"),
    "IMM": ("self.read_pc(bus, ctx) as u16", "0", "false"),
    "ZP": ("self.read_pc(bus, ctx) as u16", "0", "false"),
    "ZPX": ("self.read_pc(bus, ctx).wrapping_add(self.x) as u16", "0", "false"),
    "ZPY": ("self.read_pc(bus, ctx).wrapping_add(self.y) as u16", "0", "false"),
    "ABS": ("self.read_word_pc(bus, ctx)", "0", "false"),
    "ABSX": ("{ let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "ABSY": ("{ let base = self.read_word_pc(bus, ctx); let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "IND": ("{ let ptr = self.read_word_pc(bus, ctx); if ptr & 0x00FF == 0x00FF { (self.read(bus, ctx, ptr & 0xFF00) as u16) << 8 | self.read(bus, ctx, ptr) as u16 } else { self.read_word(bus, ctx, ptr) } }", "0", "false"),
    "INDX": ("{ let ptr = self.read_pc(bus, ctx).wrapping_add(self.x); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo }", "0", "false"),
    "INDY": ("{ let ptr = self.read_pc(bus, ctx); let lo = self.read(bus, ctx, ptr as u16) as u16; let hi = self.read(bus, ctx, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(self.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "REL": ("self.read_pc(bus, ctx) as u16", "0", "false"),
}

with open("build_cpu.py", "w") as f:
    pass

import sys
def main():
    out = []
    out.append("""use crate::bus::Bus;

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
        }
    }

    pub fn reset<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        self.pending_interrupt = Some(Interrupt::Reset);
        self.step(bus, ctx);
    }
    
    #[inline(always)]
    fn read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u8 {
        bus.read(addr, ctx)
    }

    #[inline(always)]
    fn write<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16, val: u8) {
        bus.write(addr, val, ctx)
    }

    #[inline(always)]
    fn read_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        let val = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    #[inline(always)]
    fn read_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, addr: u16) -> u16 {
        let lo = bus.read(addr, ctx) as u16;
        let hi = bus.read(addr.wrapping_add(1), ctx) as u16;
        (hi << 8) | lo
    }

    #[inline(always)]
    fn read_word_pc<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.read_pc(bus, ctx) as u16;
        let hi = self.read_pc(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[inline(always)]
    fn push<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u8) {
        self.write(bus, ctx, 0x0100 | (self.s as u16), val);
        self.s = self.s.wrapping_sub(1);
    }

    #[inline(always)]
    fn pop<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.read(bus, ctx, 0x0100 | (self.s as u16))
    }

    #[inline(always)]
    fn push_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, val: u16) {
        self.push(bus, ctx, (val >> 8) as u8);
        self.push(bus, ctx, val as u8);
    }

    #[inline(always)]
    fn pop_word<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) -> u16 {
        let lo = self.pop(bus, ctx) as u16;
        let hi = self.pop(bus, ctx) as u16;
        (hi << 8) | lo
    }

    #[inline(always)]
    fn set_z(&mut self, val: u8) {
        if val == 0 { self.p |= FLAG_Z; } else { self.p &= !FLAG_Z; }
    }

    #[inline(always)]
    fn set_n(&mut self, val: u8) {
        if val & 0x80 != 0 { self.p |= FLAG_N; } else { self.p &= !FLAG_N; }
    }

    #[inline(always)]
    fn set_zn(&mut self, val: u8) {
        self.set_z(val);
        self.set_n(val);
    }
    
    #[unsafe(link_section = ".data.ram_func")]
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

        let opcode = self.read_pc(bus, ctx);
        let mut cycles = 0;
        let mut page_cross = false;

        match opcode {
""")

    # We iterate over 0..256 to build the match
    op_map = {}
    for inst, ops in instructions.items():
        for opc, mode, cyc, pg in ops:
            op_map[opc] = (inst, mode, cyc, pg)

    for i in range(256):
        if i not in op_map:
            out.append(f"            0x{i:02X} => {{ self.read_pc(bus, ctx); 2 }} // Undocumented NOP/Ignored")
            continue
        
        inst, mode, cyc, pg = op_map[i]
        
        addr_expr, extra_cyc, check_page = modes_fetch[mode]
        
        lines = []
        lines.append(f"            0x{i:02X} => {{")
        lines.append(f"                let addr = {addr_expr};")
        if mode == "REL":
            lines.append("                let rel = addr as i8;")
        
        if inst in ["ADC", "SBC"]:
            lines.append(f"                let val = " + ("self.a" if mode == "ACC" else ("addr as u8" if mode == "IMM" else "self.read(bus, ctx, addr)")) + ";")
            lines.append("                if self.p & FLAG_D != 0 {")
            if inst == "ADC":
                lines.append("""
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
                self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
                """)
            else: # SBC
                lines.append("""
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
                """)
            lines.append("                } else {")
            if inst == "ADC":
                lines.append("""
                let a = self.a as u16; let b = val as u16; let c = (self.p & FLAG_C) as u16;
                let res = a + b + c;
                self.p &= !0xC3;
                if res > 0xFF { self.p |= FLAG_C; }
                let r = res as u8;
                if r == 0 { self.p |= FLAG_Z; }
                if (r & 0x80) != 0 { self.p |= FLAG_N; }
                if ((a ^ res) & (b ^ res) & 0x80) != 0 { self.p |= FLAG_V; }
                self.a = r;
                """)
            else: # SBC
                lines.append("""
                let a = self.a as u16; let b = (!val) as u16; let c = (self.p & FLAG_C) as u16;
                    let res = a + b + c;
                    self.p &= !0xC3;
                    if res > 0xFF { self.p |= FLAG_C; }
                    let r = res as u8;
                    if r == 0 { self.p |= FLAG_Z; }
                    if (r & 0x80) != 0 { self.p |= FLAG_N; }
                    if ((a ^ res) & (b ^ res) & 0x80) != 0 { self.p |= FLAG_V; }
                    self.a = r;
                    """)
            lines.append("                }")
        elif inst in ["AND", "EOR", "ORA", "LDA", "LDX", "LDY", "CMP", "CPX", "CPY", "BIT"]:
            lines.append(f"                let val = " + ("self.a" if mode == "ACC" else ("addr as u8" if mode == "IMM" else "self.read(bus, ctx, addr)")) + ";")
            if inst == "AND":
                lines.append("                self.a &= val; self.set_zn(self.a);")
            elif inst == "EOR":
                lines.append("                self.a ^= val; self.set_zn(self.a);")
            elif inst == "ORA":
                lines.append("                self.a |= val; self.set_zn(self.a);")
            elif inst == "LDA":
                lines.append("                self.a = val; self.set_zn(self.a);")
            elif inst == "LDX":
                lines.append("                self.x = val; self.set_zn(self.x);")
            elif inst == "LDY":
                lines.append("                self.y = val; self.set_zn(self.y);")
            elif inst in ["CMP", "CPX", "CPY"]:
                reg = "self.a" if inst == "CMP" else ("self.x" if inst == "CPX" else "self.y")
                lines.append(f"                let diff = ({reg} as u16).wrapping_sub(val as u16);")
                lines.append(f"                if {reg} >= val {{ self.p |= FLAG_C; }} else {{ self.p &= !FLAG_C; }}")
                lines.append(f"                self.set_zn(diff as u8);")
            elif inst == "BIT":
                lines.append("                let res = self.a & val;")
                lines.append("                self.set_z(res);")
                lines.append("                self.p = (self.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));")
        elif inst in ["STA", "STX", "STY"]:
            reg = "self.a" if inst == "STA" else ("self.x" if inst == "STX" else "self.y")
            lines.append(f"                self.write(bus, ctx, addr, {reg});")
        elif inst in ["DEC", "INC", "ASL", "LSR", "ROL", "ROR"]:
            if mode == "ACC":
                if inst == "ASL":
                    lines.append("                if self.a & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                self.a <<= 1;")
                    lines.append("                self.set_zn(self.a);")
                elif inst == "LSR":
                    lines.append("                if self.a & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                self.a >>= 1;")
                    lines.append("                self.set_zn(self.a);")
                elif inst == "ROL":
                    lines.append("                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };")
                    lines.append("                if self.a & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                self.a = (self.a << 1) | c;")
                    lines.append("                self.set_zn(self.a);")
                elif inst == "ROR":
                    lines.append("                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };")
                    lines.append("                if self.a & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                self.a = (self.a >> 1) | c;")
                    lines.append("                self.set_zn(self.a);")
            else:
                lines.append(f"                let mut val = self.read(bus, ctx, addr);")
                if inst == "DEC":
                    lines.append("                val = val.wrapping_sub(1);")
                elif inst == "INC":
                    lines.append("                val = val.wrapping_add(1);")
                elif inst == "ASL":
                    lines.append("                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                val <<= 1;")
                elif inst == "LSR":
                    lines.append("                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                val >>= 1;")
                elif inst == "ROL":
                    lines.append("                let c = if self.p & FLAG_C != 0 { 1 } else { 0 };")
                    lines.append("                if val & 0x80 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                val = (val << 1) | c;")
                elif inst == "ROR":
                    lines.append("                let c = if self.p & FLAG_C != 0 { 0x80 } else { 0 };")
                    lines.append("                if val & 0x01 != 0 { self.p |= FLAG_C; } else { self.p &= !FLAG_C; }")
                    lines.append("                val = (val >> 1) | c;")
                lines.append("                self.write(bus, ctx, addr, val);")
                lines.append("                self.set_zn(val);")
        elif inst in ["BCC", "BCS", "BEQ", "BMI", "BNE", "BPL", "BVC", "BVS"]:
            cond = {
                "BCC": "self.p & FLAG_C == 0",
                "BCS": "self.p & FLAG_C != 0",
                "BEQ": "self.p & FLAG_Z != 0",
                "BMI": "self.p & FLAG_N != 0",
                "BNE": "self.p & FLAG_Z == 0",
                "BPL": "self.p & FLAG_N == 0",
                "BVC": "self.p & FLAG_V == 0",
                "BVS": "self.p & FLAG_V != 0"
            }[inst]
            lines.append(f"                if {cond} {{")
            lines.append("                    let new_pc = self.pc.wrapping_add(rel as u16);")
            lines.append("                    cycles += if (new_pc & 0xFF00) != (self.pc & 0xFF00) { 2 } else { 1 };")
            lines.append("                    self.pc = new_pc;")
            lines.append("                }")
        elif inst == "BRK":
            lines.append("                self.pc = self.pc.wrapping_add(1);")
            lines.append("                self.push_word(bus, ctx, self.pc);")
            lines.append("                self.push(bus, ctx, self.p | FLAG_B | FLAG_U);")
            lines.append("                self.p |= FLAG_I;")
            lines.append("                self.pc = self.read_word(bus, ctx, 0xFFFE);")
        elif inst == "JMP":
            lines.append("                self.pc = addr;")
        elif inst == "JSR":
            lines.append("                self.push_word(bus, ctx, self.pc.wrapping_sub(1));")
            lines.append("                self.pc = addr;")
        elif inst == "RTI":
            lines.append("                self.p = self.pop(bus, ctx) | FLAG_U;")
            lines.append("                self.pc = self.pop_word(bus, ctx);")
        elif inst == "RTS":
            lines.append("                self.pc = self.pop_word(bus, ctx).wrapping_add(1);")
        elif inst in ["TAX", "TAY", "TSX", "TXA", "TXS", "TYA", "INX", "INY", "DEX", "DEY"]:
            op = {
                "TAX": "self.x = self.a;",
                "TAY": "self.y = self.a;",
                "TSX": "self.x = self.s;",
                "TXA": "self.a = self.x;",
                "TXS": "self.s = self.x;", # TXS doesn't set flags
                "TYA": "self.a = self.y;",
                "INX": "self.x = self.x.wrapping_add(1);",
                "INY": "self.y = self.y.wrapping_add(1);",
                "DEX": "self.x = self.x.wrapping_sub(1);",
                "DEY": "self.y = self.y.wrapping_sub(1);"
            }[inst]
            lines.append(f"                {op}")
            if inst != "TXS":
                reg = "self.x" if inst in ["TAX", "TSX", "INX", "DEX"] else ("self.y" if inst in ["TAY", "INY", "DEY"] else "self.a")
                lines.append(f"                self.set_zn({reg});")
        elif inst in ["CLC", "CLD", "CLI", "CLV", "SEC", "SED", "SEI"]:
            flag = {"CLC": "FLAG_C", "CLD": "FLAG_D", "CLI": "FLAG_I", "CLV": "FLAG_V", "SEC": "FLAG_C", "SED": "FLAG_D", "SEI": "FLAG_I"}[inst]
            if inst.startswith("CL"):
                lines.append(f"                self.p &= !{flag};")
            else:
                lines.append(f"                self.p |= {flag};")
        elif inst == "NOP":
            pass
        elif inst == "PHA":
            lines.append("                self.push(bus, ctx, self.a);")
        elif inst == "PHP":
            lines.append("                self.push(bus, ctx, self.p | FLAG_B | FLAG_U);")
        elif inst == "PLA":
            lines.append("                self.a = self.pop(bus, ctx);")
            lines.append("                self.set_zn(self.a);")
        elif inst == "PLP":
            lines.append("                self.p = self.pop(bus, ctx) | FLAG_U;")
            
        lines.append(f"                cycles += {cyc};")
        if pg > 0 and check_page != "false":
            lines.append(f"                if page_cross {{ cycles += {pg}; }}")
            
        lines.append(f"                cycles")
        lines.append("            }")
        out.extend(lines)

    out.append("""        }
    }
}
""")
    with open("src/cpu.rs", "w") as f:
        f.write("\n".join(out))

if __name__ == "__main__":
    main()
