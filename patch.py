with open("crates/pico1541-common/src/formats.rs", "r") as f:
    lines = f.readlines()

new_lines = []
for line in lines:
    if "let sector = ((physical_sector as u32 * 5) % (sectors as u32)) as u8;" in line:
        new_lines.append("                let sector = physical_sector as u8;\n")
    elif "let sector = (physical_sector * 5) % sectors;" in line:
        new_lines.append("            let sector = physical_sector;\n")
    else:
        new_lines.append(line)

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.writelines(new_lines)
