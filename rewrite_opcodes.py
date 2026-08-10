import re

with open("crates/racc6502v4/src/opcodes.rs", "r") as f:
    content = f.read()

# Helper to find an opcode's body and replace it with another opcode's body (with modifications)
def replace_opcode(target_op, source_op, modifier_func):
    global content
    # Find source opcode body
    m_src = re.search(f"pub fn op_{source_op}<B: Bus>\\(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx\\) -> u32 {{(.*?)^\}}", content, re.MULTILINE | re.DOTALL)
    if not m_src:
        print(f"Source {source_op} not found")
        return
    
    src_body = m_src.group(1)
    new_body = modifier_func(src_body)
    
    # Replace target opcode body
    pattern = f"(pub fn op_{target_op}<B: Bus>\\(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx\\) -> u32 {{).*?(^\}})"
    
    new_content = re.sub(pattern, f"\\1{new_body}\\2", content, flags=re.MULTILINE | re.DOTALL)
    if new_content == content:
        print(f"Target {target_op} not modified (maybe already done or regex failed)")
    content = new_content

def mod_lax(body):
    return body.replace("cpu.a = val;", "cpu.a = val;\n    cpu.x = val;")

def mod_sax(body):
    # Source is STA, body has: cpu.write(bus, ctx, offset, addr, cpu.a);
    return body.replace("cpu.write(bus, ctx, offset, addr, cpu.a);", "cpu.write(bus, ctx, offset, addr, cpu.a & cpu.x);")

# LAX: A3 (LDA A1), A7 (LDA A5), AB (LDA A9), AF (LDA AD), B3 (LDA B1), B7 (LDA B5), BF (LDA BD)
replace_opcode("a3", "a1", mod_lax)
replace_opcode("a7", "a5", mod_lax)
replace_opcode("ab", "a9", mod_lax)
replace_opcode("af", "ad", mod_lax)
replace_opcode("b3", "b1", mod_lax)
replace_opcode("b7", "b5", mod_lax)
replace_opcode("bf", "bd", mod_lax)

# SAX: 83 (STA 81), 87 (STA 85), 8f (STA 8d), 97 (STA 95)
replace_opcode("83", "81", mod_sax)
replace_opcode("87", "85", mod_sax)
replace_opcode("8f", "8d", mod_sax)
replace_opcode("97", "95", mod_sax)

with open("crates/racc6502v4/src/opcodes.rs", "w") as f:
    f.write(content)

print("Done rewriting opcodes.rs")
