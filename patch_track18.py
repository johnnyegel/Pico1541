with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

content = content.replace("let interleave = if sectors == 18 { 11 } else { 10 };", "let interleave = if track == 18 { 3 } else if sectors == 18 { 11 } else { 10 };")

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
