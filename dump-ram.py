#!/usr/bin/env python3
import subprocess
import sys
import os
import struct

ELF_PATHS = [
    "target/thumbv6m-none-eabi/release/pico1541-rp2040",
    "target/thumbv6m-none-eabi/debug/pico1541-rp2040"
]

def get_ram_pointer_address(nm_path):
    for path in ELF_PATHS:
        if not os.path.exists(path): continue
        result = subprocess.run([nm_path, path], capture_output=True, text=True)
        if result.returncode != 0: continue
        for line in result.stdout.splitlines():
            if "RAM_POINTER" in line:
                addr_hex = line.split()[0]
                return int(addr_hex, 16)
    return None

def read_memory_binary(addr, size, filename):
    cmd = ["probe-rs", "read", "b8", str(addr), str(size), "--output", filename, "-f", "binary", "--chip", "RP2040"]
    result = subprocess.run(cmd)
    if result.returncode != 0:
        print(f"Error running probe-rs", file=sys.stderr)
        sys.exit(1)
    with open(filename, "rb") as f:
        return f.read()

def main():
    nm_path = "nm"
    addr = get_ram_pointer_address(nm_path)
    if addr is None:
        print("Could not find RAM_POINTER in ELF.", file=sys.stderr)
        sys.exit(1)
        
    print(f"Found RAM_POINTER at {hex(addr)}")
    ptr_data = read_memory_binary(addr, 4, "/tmp/ram_ptr.bin")
    ram_addr = struct.unpack("<I", ptr_data)[0]
    
    if ram_addr == 0:
        print("RAM_POINTER is null! Emulator hasn't ticked yet?", file=sys.stderr)
        sys.exit(1)
        
    print(f"RAM is at {hex(ram_addr)}. Dumping 65536 bytes...")
    read_memory_binary(ram_addr, 65536, "ram_dump.bin")
    
    print("Dumped to ram_dump.bin")

if __name__ == "__main__":
    main()
