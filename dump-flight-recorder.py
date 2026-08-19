#!/usr/bin/env python3

import subprocess
import sys
import struct
import os
import shutil

# Configuration
ELF_PATHS = [
    "target/thumbv6m-none-eabi/release/pico1541-rp2040-main",
    "target/thumbv6m-none-eabi/debug/pico1541-rp2040-main"
]
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
    for path in ELF_PATHS:
        if not os.path.exists(path):
            continue
            
        result = subprocess.run([nm_path, path], capture_output=True, text=True)
        if result.returncode != 0:
            continue
            
        for line in result.stdout.splitlines():
            if "FLIGHT_RECORDER" in line:
                addr_hex = line.split()[0]
                print(f"Using ELF: {path}")
                return int(addr_hex, 16)
                
    print(f"Error: Could not find FLIGHT_RECORDER symbol in any ELF: {ELF_PATHS}", file=sys.stderr)
    sys.exit(1)

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Dump flight recorder")
    parser.add_argument("-l", "--last", type=int, default=0, help="Only show the last N records")
    parser.add_argument("--hex", action="store_true", help="Show raw hex bytes for each record")
    parser.add_argument("-f", "--file", type=str, help="Read from an existing binary file instead of using probe-rs")
    parser.add_argument("--offset", type=int, default=0, help="Byte offset into the buffer to start reading (default: 0)")
    args = parser.parse_args()
    
    if args.file:
        bin_out = args.file
        print(f"Reading from file {args.file}...")
        try:
            with open(args.file, "rb") as f:
                data = f.read()
        except Exception as e:
            print(f"Error reading file: {e}")
            sys.exit(1)
    else:
        nm_path = find_nm()
        addr = get_flight_recorder_address(nm_path)
        print(f"Found FLIGHT_RECORDER at address: 0x{addr:X}")
        
        # Read 8212 bytes (buffer + last_record + index)
        total_size = 8212
        print(f"Reading {total_size} bytes via probe-rs...")
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
    
    # Try to find current_index at various possible offsets, default to 0 if invalid
    current_index = 0
    # Usually index is after the 8192 byte buffer, but struct layout may vary
    for potential_idx_offset in [8192, 8208, 0, 16]:
        if potential_idx_offset + 4 <= len(data):
            idx_val = struct.unpack("<I", data[potential_idx_offset:potential_idx_offset+4])[0]
            if idx_val < 512 and idx_val > 0:
                current_index = idx_val
                break

    print("\n--- FLIGHT RECORDER DUMP (Current Index: {}) ---".format(current_index))
    print(f"Using record data offset: {args.offset}")
    
    records = []
    
    for i in range(512):
        rec_start = args.offset + (i * 16)
        if rec_start + 16 > len(data):
            break
        record_data = data[rec_start:rec_start+16]
        
        # Layout: cycle(4), pc(4), event(1), via1(1), via2(1), pcr1(1), pcr2(1), pa2(1), sync(1), padding(1)
        cycle, pc, evt, v1, v2, pcr1, pcr2, pa2, sync = struct.unpack("<IIBBBBBBB1x", record_data)
        
        records.append({
            "idx": i,
            "cycle": cycle,
            "pc": pc,
            "evt": evt,
            "v1": v1,
            "v2": v2,
            "pcr1": pcr1,
            "pcr2": pcr2,
            "pa2": pa2,
            "sync": sync,
            "raw": record_data
        })
    
    # Sort records by cycle count to ensure chronological order regardless of ring wrap
    valid_records = [r for r in records if r["cycle"] > 0 or r["pc"] > 0]
    
    if args.last > 0:
        valid_records = valid_records[-args.last:]
        
    printed_any = False
    for r in valid_records:
        printed_any = True
        hex_str = ""
        if args.hex:
            hex_str = " | HEX: " + " ".join([f"{b:02X}" for b in r["raw"]])
            
        print(f"Cycle: {r['cycle']:10d} | PC: {r['pc']:04X} | Event: {r['evt']:3d} | VIA1: {r['v1']:02X} (PCR: {r['pcr1']:02X}) | VIA2: PB={r['v2']:02X} PA={r['pa2']:02X} SYNC={r['sync']} (PCR: {r['pcr2']:02X}){hex_str}")
        
    if not printed_any:
        print("(Flight recorder is empty)")

    print("\n--- END FLIGHT RECORDER ---")
    
if __name__ == "__main__":
    main()
