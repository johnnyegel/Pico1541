with open("crates/pico1541-common/src/formats.rs", "r") as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if "let sector = physical_sector;" in line:
        new_lines.append("            let interleave = if sectors == 18 { 11 } else { 10 };\n")
        new_lines.append("            let sector = (physical_sector * interleave) % sectors;\n")
    elif "let sector = physical_sector as u8;" in line:
        new_lines.append("                let interleave = if sectors == 18 { 11 } else { 10 };\n")
        new_lines.append("                let sector = ((physical_sector * interleave) % sectors) as u8;\n")
    else:
        new_lines.append(line)

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.writelines(new_lines)
