#!/usr/bin/env python3
import subprocess
import sys
import os
import struct

CHIP = "rp235x"

ELF_PATHS = [
    f"target/thumbv8m.main-none-eabihf/release/pico1541-rp2350",
    f"target/thumbv8m.main-none-eabihf/debug/pico1541-rp2350",
    f"target/thumbv6m-none-eabi/release/pico1541-rp2040",
    f"target/thumbv6m-none-eabi/debug/pico1541-rp2040"
]

def get_ram_pointer_address(nm_path):
    for path in ELF_PATHS:
        if not os.path.exists(path): continue
        result = subprocess.run([nm_path, path], capture_output=True, text=True)
        if result.returncode != 0: continue
        for line in result.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[-1] == "RAM_POINTER":
                addr_hex = parts[0]
                return int(addr_hex, 16)
    return None

def get_sys_memory_address(nm_path):
    for path in ELF_PATHS:
        if not os.path.exists(path): continue
        result = subprocess.run([nm_path, path], capture_output=True, text=True)
        if result.returncode != 0: continue
        for line in result.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 3 and "SYS_MEMORY" in parts[-1]:
                addr_hex = parts[0]
                return int(addr_hex, 16)
    return None

def read_memory_binary(addr, size, filename):
    cmd = ["probe-rs", "read", "b8", str(addr), str(size), "--output", filename, "-f", "binary", "--chip", CHIP]
    result = subprocess.run(cmd)
    if result.returncode != 0:
        print(f"Error running probe-rs", file=sys.stderr)
        sys.exit(1)
    with open(filename, "rb") as f:
        return f.read()

def main():
    print(f"Target CHIP is {CHIP}")

    nm_path = "nm"
    if len(sys.argv) > 1:
        addr = int(sys.argv[1], 0)
        print(f"Using provided address: {hex(addr)}")
        # Check if caller passed SYS_MEMORY directly or RAM_POINTER
        # If it looks like CORE1 RAM (>= 0x20040000), use directly
        if addr >= 0x20040000:
            ram_addr = addr
        else:
            ptr_data = read_memory_binary(addr, 4, "/tmp/ram_ptr.bin")
            ram_addr = struct.unpack("<I", ptr_data)[0]
    else:
        sys_mem_addr = get_sys_memory_address(nm_path)
        if sys_mem_addr is not None:
            print(f"Found SYS_MEMORY directly at {hex(sys_mem_addr)}")
            ram_addr = sys_mem_addr
        else:
            addr = get_ram_pointer_address(nm_path)
            if addr is None:
                print("Could not find SYS_MEMORY or RAM_POINTER in ELF.", file=sys.stderr)
                print("Usage: ./dump-ram.py [SYS_MEMORY_OR_RAM_POINTER_ADDRESS]", file=sys.stderr)
                sys.exit(1)
            print(f"Found RAM_POINTER at {hex(addr)}")
            ptr_data = read_memory_binary(addr, 4, "/tmp/ram_ptr.bin")
            ram_addr = struct.unpack("<I", ptr_data)[0]
    
    if ram_addr == 0:
        print("RAM address is null!", file=sys.stderr)
        sys.exit(1)
        
    print(f"RAM is at {hex(ram_addr)}. Dumping 32768 bytes (SystemBus RAM size)...")
    read_memory_binary(ram_addr, 32768, "ram_dump.bin")
    
    print("Dumped to ram_dump.bin")

if __name__ == "__main__":
    main()
