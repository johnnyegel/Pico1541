import re

with open("crates/pico1541-common/src/formats.rs", "r") as f:
    content = f.read()

# Replace dest.fill(0x55) with nothing, and add padding at the end of the track!
content = content.replace("dest.fill(0x55);", "")

# VirtualDiskImage padding
content = re.sub(
    r"(for _ in 0\.\.gap_per_sector \{\s*if write_ptr < dest\.len\(\) \{\s*dest\[write_ptr\] = 0x55;\s*write_ptr \+= 1;\s*\}\s*\})",
    r"\1\n        while write_ptr < speed_zone_len && write_ptr < dest.len() {\n            dest[write_ptr] = 0x55;\n            write_ptr += 1;\n        }",
    content,
    count=1
)

# D64Image padding
content = re.sub(
    r"(for _ in 0\.\.gap_per_sector \{\s*if write_ptr < dest\.len\(\) \{\s*dest\[write_ptr\] = 0x55;\s*write_ptr \+= 1;\s*\}\s*\})",
    r"\1\n        while write_ptr < speed_zone_len && write_ptr < dest.len() {\n            dest[write_ptr] = 0x55;\n            write_ptr += 1;\n        }",
    content,
    count=0 # Apply to all
)

with open("crates/pico1541-common/src/formats.rs", "w") as f:
    f.write(content)
