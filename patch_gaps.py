with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

# Change gap bytes from 8 to 16
content = content.replace("for _ in 0..8 {", "for _ in 0..16 {")
# Change 0xAA to 0x55 for better synchronization reliability
content = content.replace("dest[write_ptr] = 0xAA;", "dest[write_ptr] = 0x55;")

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
