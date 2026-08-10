import re

with open("crates/racc6502v4/src/opcodes.rs", "r") as f:
    content = f.read()

def replace_opcode(target_op, source_op, modifier_func):
    global content
    m_src = re.search(f"pub fn op_{source_op}<B: Bus>\\(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx\\) -> u32 {{(.*?)^\}}", content, re.MULTILINE | re.DOTALL)
    if not m_src: return
    
    src_body = m_src.group(1)
    new_body = modifier_func(src_body)
    
    pattern = f"(pub fn op_{target_op}<B: Bus>\\(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx\\) -> u32 {{).*?(^\}})"
    content = re.sub(pattern, f"\\1{new_body}\\2", content, flags=re.MULTILINE | re.DOTALL)

def mod_dcp(body):
    # DEC (C6) -> DCP (DEC then CMP)
    # C6 body has:
    # let val = cpu.read(bus, ctx, offset, addr).wrapping_sub(1);
    # cpu.write(bus, ctx, offset, addr, val);
    # cpu.set_zn(val);
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    let diff = (cpu.a as u16).wrapping_sub(val as u16);\n    cpu.p &= !0x01;\n    if diff < 0x100 { cpu.p |= 0x01; }\n    cpu.set_zn(diff as u8);")
    return b

def mod_isc(body):
    # INC (E6) -> ISC (INC then SBC)
    # E6 body has:
    # let val = cpu.read(bus, ctx, offset, addr).wrapping_add(1);
    # cpu.write(bus, ctx, offset, addr, val);
    # cpu.set_zn(val);
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    let a = cpu.a as u16;\n    let b_val = (val ^ 0xFF) as u16;\n    let c = (cpu.p & 0x01) as u16;\n    let sum = a + b_val + c;\n    cpu.p &= !(0x01 | 0x40);\n    if sum > 0xFF { cpu.p |= 0x01; }\n    if ((a ^ sum) & (b_val ^ sum) & 0x80) != 0 { cpu.p |= 0x40; }\n    cpu.a = sum as u8;\n    cpu.set_zn(cpu.a);")
    return b

def mod_slo(body):
    # ASL (06) -> SLO (ASL then ORA)
    # 06 body has:
    # let v = cpu.read(bus, ctx, offset, addr);
    # let val = v << 1;
    # cpu.p &= !0x01; if (v & 0x80) != 0 { cpu.p |= 0x01; }
    # cpu.write(bus, ctx, offset, addr, val);
    # cpu.set_zn(val);
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    cpu.a |= val;\n    cpu.set_zn(cpu.a);")
    return b

def mod_rla(body):
    # ROL (26) -> RLA (ROL then AND)
    # 26 body has:
    # cpu.set_zn(val);
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    cpu.a &= val;\n    cpu.set_zn(cpu.a);")
    return b

def mod_sre(body):
    # LSR (46) -> SRE (LSR then EOR)
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    cpu.a ^= val;\n    cpu.set_zn(cpu.a);")
    return b

def mod_rra(body):
    # ROR (66) -> RRA (ROR then ADC)
    b = body.replace("cpu.set_zn(val);", "cpu.set_zn(val);\n    let a = cpu.a as u16;\n    let b_val = val as u16;\n    let c = (cpu.p & 0x01) as u16;\n    let sum = a + b_val + c;\n    cpu.p &= !(0x01 | 0x40);\n    if sum > 0xFF { cpu.p |= 0x01; }\n    if ((a ^ sum) & (b_val ^ sum) & 0x80) != 0 { cpu.p |= 0x40; }\n    cpu.a = sum as u8;\n    cpu.set_zn(cpu.a);")
    return b


# DCP
replace_opcode("c3", "c6", mod_dcp)
replace_opcode("c7", "c6", mod_dcp)
replace_opcode("cf", "ce", mod_dcp)
replace_opcode("d3", "d6", mod_dcp)
replace_opcode("d7", "d6", mod_dcp)
replace_opcode("db", "de", mod_dcp) # d6->db not exact, db is ABS Y, de is ABS X. I will just use DE as base for DB (ABS Y instead of ABS X). Wait, DB is ABS Y. DE is ABS X. It will use the X index instead of Y! I'll skip the others and just do the exact base matches.

# It's better to match the EXACT addressing mode.
def do_repl(target, base_opcode, modifier):
    replace_opcode(target, base_opcode, modifier)

do_repl("c3", "c1", mod_dcp) # (IND, X)
do_repl("c7", "c5", mod_dcp) # ZP
do_repl("cf", "cd", mod_dcp) # ABS
do_repl("d3", "d1", mod_dcp) # (IND), Y
do_repl("d7", "d5", mod_dcp) # ZP, X
do_repl("db", "d9", mod_dcp) # ABS, Y
do_repl("df", "dd", mod_dcp) # ABS, X

do_repl("e3", "e1", mod_isc)
do_repl("e7", "e5", mod_isc)
do_repl("ef", "ed", mod_isc)
do_repl("f3", "f1", mod_isc)
do_repl("f7", "f5", mod_isc)
do_repl("fb", "f9", mod_isc)
do_repl("ff", "fd", mod_isc)

do_repl("03", "01", mod_slo)
do_repl("07", "05", mod_slo)
do_repl("0f", "0d", mod_slo)
do_repl("13", "11", mod_slo)
do_repl("17", "15", mod_slo)
do_repl("1b", "19", mod_slo)
do_repl("1f", "1d", mod_slo)

do_repl("23", "21", mod_rla)
do_repl("27", "25", mod_rla)
do_repl("2f", "2d", mod_rla)
do_repl("33", "31", mod_rla)
do_repl("37", "35", mod_rla)
do_repl("3b", "39", mod_rla)
do_repl("3f", "3d", mod_rla)

do_repl("43", "41", mod_sre)
do_repl("47", "45", mod_sre)
do_repl("4f", "4d", mod_sre)
do_repl("53", "51", mod_sre)
do_repl("57", "55", mod_sre)
do_repl("5b", "59", mod_sre)
do_repl("5f", "5d", mod_sre)

do_repl("63", "61", mod_rra)
do_repl("67", "65", mod_rra)
do_repl("6f", "6d", mod_rra)
do_repl("73", "71", mod_rra)
do_repl("77", "75", mod_rra)
do_repl("7b", "79", mod_rra)
do_repl("7f", "7d", mod_rra)

with open("crates/racc6502v4/src/opcodes.rs", "w") as f:
    f.write(content)

