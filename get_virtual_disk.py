with open("crates/pico1541-common/src/formats.rs", "r") as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "fn encode_file_sectors" in line or "struct VirtualDiskImage" in line:
        pass
# Actually, I'll just use grep to find the file reading part in VirtualDiskImage
