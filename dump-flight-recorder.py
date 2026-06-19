#!/usr/bin/env python3

import subprocess
import sys
import struct
import os
import shutil

# Configuration
ELF_PATH = "target/thumbv6m-none-eabi/release/pico1541-rp2040"
NM_CANDIDATES = [
    "nm",
    "arm-none-eabi-nm",
    os.path.expanduser("~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-nm")
]

def find_nm():
    for nm in NM_CANDIDATES:
        if shutil.which(nm):
            return nm
    print("Error: Could not find nm or llvm-nm. Please install binutils or rust llvm-tools-preview.", file=sys.stderr)
    sys.exit(1)

def get_flight_recorder_address(nm_path):
    if not os.path.exists(ELF_PATH):
        print(f"Error: ELF file not found at {ELF_PATH}", file=sys.stderr)
        print("Please build the firmware first: cargo build --package pico1541-rp2040 --target thumbv6m-none-eabi --release", file=sys.stderr)
        sys.exit(1)
        
    result = subprocess.run([nm_path, ELF_PATH], capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error running {nm_path}", file=sys.stderr)
        sys.exit(1)
        
    for line in result.stdout.splitlines():
        if "FLIGHT_RECORDER" in line:
            addr_hex = line.split()[0]
            return int(addr_hex, 16)
            
    print("Error: Could not find FLIGHT_RECORDER symbol in ELF.", file=sys.stderr)
    sys.exit(1)

def main():
    nm_path = find_nm()
    addr = get_flight_recorder_address(nm_path)
    print(f"Found FLIGHT_RECORDER at address: 0x{addr:08X}")
    
    # Calculate sizes based on #[repr(C)]
    # FlightRecord: cycle(u32: 4), pc(u32: 4), event_type(u8: 1), via1(u8: 1), via2(u8: 1), pcr1(u8: 1), pcr2(u8: 1), _padding(3) = 16 bytes
    # buffer: [FlightRecord; 512] = 512 * 16 = 8192 bytes
    # index: usize(4 bytes)
    # Total size = 8196 bytes
    total_size = 8196
    
    print(f"Reading {total_size} bytes via probe-rs...")
    # probe-rs read b8 <ADDR> <WORDS> --output /tmp/fr.bin -f binary
    bin_out = "/tmp/fr.bin"
    result = subprocess.run([
        "probe-rs", "read", "b8", f"0x{addr:08X}", str(total_size),
        "--output", bin_out, "-f", "binary", "--chip", "RP2040"
    ])
    
    if result.returncode != 0:
        print("Error: probe-rs failed to read memory.", file=sys.stderr)
        sys.exit(1)
        
    with open(bin_out, "rb") as f:
        data = f.read()
        
    if len(data) < total_size:
        print("Error: Read incomplete data.", file=sys.stderr)
        sys.exit(1)
        
    # Read the buffer
    records = []
    for i in range(512):
        offset = i * 16
        record_data = data[offset:offset+16]
        # <I I B B B B B 3x
        cycle, pc, evt, v1, v2, pcr1, pcr2 = struct.unpack("<IIBBBBB3x", record_data)
        records.append({
            "cycle": cycle,
            "pc": pc,
            "event": evt,
            "via1": v1,
            "via2": v2,
            "pcr1": pcr1,
            "pcr2": pcr2
        })
        
    # Read the index
    index_data = data[8192:8196]
    current_index = struct.unpack("<I", index_data)[0]
    
    print(f"\n--- FLIGHT RECORDER DUMP (Current Index: {current_index}) ---\n")
    
    printed_any = False
    for i in range(512):
        idx = (current_index + i) % 512
        r = records[idx]
        if r["cycle"] > 0:
            printed_any = True
            print(f"Cycle: {r['cycle']:10d} | PC: {r['pc']:04X} | Event: {r['event']:3d} | VIA1: {r['via1']:02X} (PCR: {r['pcr1']:02X}) | VIA2: {r['via2']:02X} (PCR: {r['pcr2']:02X})")
            
    if not printed_any:
        print("(Flight recorder is empty)")

    print("\n--- END FLIGHT RECORDER ---")
    
if __name__ == "__main__":
    main()
