import json

opcodes = {}

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
    "IMM": ("cpu.read_pc(bus, ctx, &mut offset) as u16", "0", "false"),
    "ZP": ("cpu.read_pc(bus, ctx, &mut offset) as u16", "0", "false"),
    "ZPX": ("cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16", "0", "false"),
    "ZPY": ("cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.y) as u16", "0", "false"),
    "ABS": ("cpu.read_word_pc(bus, ctx, &mut offset)", "0", "false"),
    "ABSX": ("{ let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "ABSY": ("{ let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "IND": ("{ let ptr = cpu.read_word_pc(bus, ctx, &mut offset); if ptr & 0x00FF == 0x00FF { (cpu.read(bus, ctx, &mut offset, ptr & 0xFF00) as u16) << 8 | cpu.read(bus, ctx, &mut offset, ptr) as u16 } else { cpu.read_word(bus, ctx, &mut offset, ptr) } }", "0", "false"),
    "INDX": ("{ let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo }", "0", "false"),
    "INDY": ("{ let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a }", "0", "page_cross"),
    "REL": ("cpu.read_pc(bus, ctx, &mut offset) as u16", "0", "false"),
}

def main():
    out = []
    out.append("""use crate::cpu::{CPU, FLAG_B, FLAG_C, FLAG_D, FLAG_I, FLAG_N, FLAG_U, FLAG_V, FLAG_Z};
use crate::bus::Bus;

pub type OpcodeFn<B> = fn(&mut CPU, &mut B, &mut <B as Bus>::Ctx) -> u32;
""")

    op_map = {}
    for inst, ops in instructions.items():
        for opc, mode, cyc, pg in ops:
            op_map[opc] = (inst, mode, cyc, pg)

    for i in range(256):
        out.append(f"#[allow(unused_variables, unused_assignments, unused_mut)]")
        out.append(f"pub fn op_{i:02x}<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {{")
        out.append("    let mut cycles: u32 = 0;\n    let mut offset: u32 = 1;")
        out.append("    let mut page_cross = false;\n")
        
        if i not in op_map:
            out.append(" cpu.read_pc(bus, ctx, &mut offset); 2 }")
            out.append("")
            continue
            
        inst, mode, cyc, pg = op_map[i]
        addr_expr, extra_cyc, check_page = modes_fetch[mode]
        
        lines = []
        lines.append(f"                let addr = {addr_expr};")
        if mode == "REL":
            lines.append("                let rel = addr as i8;")
        
        if inst in ["ADC", "SBC"]:
            lines.append(f"                let val = " + ("cpu.a" if mode == "ACC" else ("addr as u8" if mode == "IMM" else "cpu.read(bus, ctx, &mut offset, addr)")) + ";")
            if inst == "ADC":
                lines.append("                cpu.do_adc(val);")
            else:
                lines.append("                cpu.do_sbc(val);")
        elif inst in ["AND", "EOR", "ORA", "LDA", "LDX", "LDY", "CMP", "CPX", "CPY", "BIT"]:
            lines.append(f"                let val = " + ("cpu.a" if mode == "ACC" else ("addr as u8" if mode == "IMM" else "cpu.read(bus, ctx, &mut offset, addr)")) + ";")
            if inst == "AND":
                lines.append("                cpu.a &= val; cpu.set_zn(cpu.a);")
            elif inst == "EOR":
                lines.append("                cpu.a ^= val; cpu.set_zn(cpu.a);")
            elif inst == "ORA":
                lines.append("                cpu.a |= val; cpu.set_zn(cpu.a);")
            elif inst == "LDA":
                lines.append("                cpu.a = val; cpu.set_zn(cpu.a);")
            elif inst == "LDX":
                lines.append("                cpu.x = val; cpu.set_zn(cpu.x);")
            elif inst == "LDY":
                lines.append("                cpu.y = val; cpu.set_zn(cpu.y);")
            elif inst in ["CMP", "CPX", "CPY"]:
                reg = "cpu.a" if inst == "CMP" else ("cpu.x" if inst == "CPX" else "cpu.y")
                lines.append(f"                let diff = ({reg} as u16).wrapping_sub(val as u16);")
                lines.append(f"                if {reg} >= val {{ cpu.p |= FLAG_C; }} else {{ cpu.p &= !FLAG_C; }}")
                lines.append(f"                cpu.set_zn(diff as u8);")
            elif inst == "BIT":
                lines.append("                let res = cpu.a & val;")
                lines.append("                cpu.set_z(res);")
                lines.append("                cpu.p = (cpu.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));")
        elif inst in ["STA", "STX", "STY"]:
            reg = "cpu.a" if inst == "STA" else ("cpu.x" if inst == "STX" else "cpu.y")
            lines.append(f"                cpu.write(bus, ctx, &mut offset, addr, {reg});")
        elif inst in ["DEC", "INC", "ASL", "LSR", "ROL", "ROR"]:
            if mode == "ACC":
                if inst == "ASL":
                    lines.append("                if cpu.a & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                cpu.a <<= 1;")
                    lines.append("                cpu.set_zn(cpu.a);")
                elif inst == "LSR":
                    lines.append("                if cpu.a & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                cpu.a >>= 1;")
                    lines.append("                cpu.set_zn(cpu.a);")
                elif inst == "ROL":
                    lines.append("                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };")
                    lines.append("                if cpu.a & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                cpu.a = (cpu.a << 1) | c;")
                    lines.append("                cpu.set_zn(cpu.a);")
                elif inst == "ROR":
                    lines.append("                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };")
                    lines.append("                if cpu.a & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                cpu.a = (cpu.a >> 1) | c;")
                    lines.append("                cpu.set_zn(cpu.a);")
            else:
                lines.append(f"                let mut val = cpu.read(bus, ctx, &mut offset, addr);")
                if inst == "DEC":
                    lines.append("                val = val.wrapping_sub(1);")
                elif inst == "INC":
                    lines.append("                val = val.wrapping_add(1);")
                elif inst == "ASL":
                    lines.append("                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                val <<= 1;")
                elif inst == "LSR":
                    lines.append("                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                val >>= 1;")
                elif inst == "ROL":
                    lines.append("                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };")
                    lines.append("                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                val = (val << 1) | c;")
                elif inst == "ROR":
                    lines.append("                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };")
                    lines.append("                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }")
                    lines.append("                val = (val >> 1) | c;")
                lines.append("                cpu.write(bus, ctx, &mut offset, addr, val);")
                lines.append("                cpu.set_zn(val);")
        elif inst in ["BCC", "BCS", "BEQ", "BMI", "BNE", "BPL", "BVC", "BVS"]:
            cond = {
                "BCC": "cpu.p & FLAG_C == 0",
                "BCS": "cpu.p & FLAG_C != 0",
                "BEQ": "cpu.p & FLAG_Z != 0",
                "BMI": "cpu.p & FLAG_N != 0",
                "BNE": "cpu.p & FLAG_Z == 0",
                "BPL": "cpu.p & FLAG_N == 0",
                "BVC": "cpu.p & FLAG_V == 0",
                "BVS": "cpu.p & FLAG_V != 0"
            }[inst]
            lines.append(f"                if {cond} {{")
            lines.append("                    let new_pc = cpu.pc.wrapping_add(rel as u16);")
            lines.append("                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };")
            lines.append("                    cpu.pc = new_pc;")
            lines.append("                }")
        elif inst == "BRK":
            lines.append("                cpu.pc = cpu.pc.wrapping_add(1);")
            lines.append("                cpu.push_word(bus, ctx, &mut offset, cpu.pc);")
            lines.append("                cpu.push(bus, ctx, &mut offset, cpu.p | FLAG_B | FLAG_U);")
            lines.append("                cpu.p |= FLAG_I;")
            lines.append("                cpu.pc = cpu.read_word(bus, ctx, &mut offset, 0xFFFE);")
        elif inst == "JMP":
            lines.append("                cpu.pc = addr;")
        elif inst == "JSR":
            lines.append("                cpu.push_word(bus, ctx, &mut offset, cpu.pc.wrapping_sub(1));")
            lines.append("                cpu.pc = addr;")
        elif inst == "RTI":
            lines.append("                cpu.p = cpu.pop(bus, ctx, &mut offset) | FLAG_U;")
            lines.append("                cpu.pc = cpu.pop_word(bus, ctx, &mut offset);")
        elif inst == "RTS":
            lines.append("                cpu.pc = cpu.pop_word(bus, ctx, &mut offset).wrapping_add(1);")
        elif inst in ["TAX", "TAY", "TSX", "TXA", "TXS", "TYA", "INX", "INY", "DEX", "DEY"]:
            op = {
                "TAX": "cpu.x = cpu.a;",
                "TAY": "cpu.y = cpu.a;",
                "TSX": "cpu.x = cpu.s;",
                "TXA": "cpu.a = cpu.x;",
                "TXS": "cpu.s = cpu.x;",
                "TYA": "cpu.a = cpu.y;",
                "INX": "cpu.x = cpu.x.wrapping_add(1);",
                "INY": "cpu.y = cpu.y.wrapping_add(1);",
                "DEX": "cpu.x = cpu.x.wrapping_sub(1);",
                "DEY": "cpu.y = cpu.y.wrapping_sub(1);"
            }[inst]
            lines.append(f"                {op}")
            if inst != "TXS":
                reg = "cpu.x" if inst in ["TAX", "TSX", "INX", "DEX"] else ("cpu.y" if inst in ["TAY", "INY", "DEY"] else "cpu.a")
                lines.append(f"                cpu.set_zn({reg});")
        elif inst in ["CLC", "CLD", "CLI", "CLV", "SEC", "SED", "SEI"]:
            flag = {"CLC": "FLAG_C", "CLD": "FLAG_D", "CLI": "FLAG_I", "CLV": "FLAG_V", "SEC": "FLAG_C", "SED": "FLAG_D", "SEI": "FLAG_I"}[inst]
            if inst.startswith("CL"):
                lines.append(f"                cpu.p &= !{flag};")
            else:
                lines.append(f"                cpu.p |= {flag};")
        elif inst == "NOP":
            pass
        elif inst == "PHA":
            lines.append("                cpu.push(bus, ctx, &mut offset, cpu.a);")
        elif inst == "PHP":
            lines.append("                cpu.push(bus, ctx, &mut offset, cpu.p | FLAG_B | FLAG_U);")
        elif inst == "PLA":
            lines.append("                cpu.a = cpu.pop(bus, ctx, &mut offset);")
            lines.append("                cpu.set_zn(cpu.a);")
        elif inst == "PLP":
            lines.append("                cpu.p = cpu.pop(bus, ctx, &mut offset) | FLAG_U;")
            
        lines.append(f"                cycles += {cyc};")
        if pg > 0 and check_page != "false":
            lines.append(f"                if page_cross {{ cycles += {pg}; }}")
            
        lines.append(f"                cycles")
        lines.append("            }")
        out.extend(lines)
        out.append("")

    out.append("pub const fn get_opcodes<B: Bus>() -> [OpcodeFn<B>; 256] {")
    out.append("    [")
    for i in range(256):
        out.append(f"        op_{i:02x},")
    out.append("    ]")
    out.append("}")

    with open("crates/racc6502v4/src/opcodes.rs", "w") as f:
        f.write("\n".join(out))

if __name__ == "__main__":
    main()
