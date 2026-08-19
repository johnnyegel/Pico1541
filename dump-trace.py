#!/usr/bin/env python3
import subprocess
import sys
import os
import struct

ELF_PATHS = [
    "target/thumbv6m-none-eabi/release/pico1541-rp2040-main",
    "target/thumbv6m-none-eabi/debug/pico1541-rp2040-main",
    "target/thumbv6m-none-eabi/release/pico1541-rp2040",
    "target/thumbv6m-none-eabi/debug/pico1541-rp2040"
]

def get_symbol_address(nm_path, symbol_name):
    for path in ELF_PATHS:
        if not os.path.exists(path): continue
        result = subprocess.run([nm_path, path], capture_output=True, text=True)
        if result.returncode != 0: continue
        for line in result.stdout.splitlines():
            parts = line.split()
            if len(parts) >= 3 and symbol_name in parts[-1]:
                addr_hex = parts[0]
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
    addr = get_symbol_address(nm_path, "FLIGHT_RECORDER")
    if addr is None:
        print("Could not find FLIGHT_RECORDER in ELF. Did you build with --features \"flight-recorder\"?", file=sys.stderr)
        sys.exit(1)
        
    print(f"Found FLIGHT_RECORDER at {hex(addr)}. Dumping trace...")
    
    # 512 records * 16 bytes = 8192
    # index (usize) = 4 bytes
    # last_record = 16 bytes
    # Total = 8212 bytes
    data = read_memory_binary(addr, 8212, "/tmp/flight_recorder.bin")
    
    # Extract the buffer and index
    buffer_data = data[:8192]
    index_data = data[8192:8196]
    
    current_index = struct.unpack("<I", index_data)[0]
    if current_index >= 512:
        print(f"Warning: Corrupt index {current_index}. Assuming 0.")
        current_index = 0
        
    print(f"\n--- FLIGHT RECORDER TRACE (Latest {512} events, chronologically) ---")
    print(f"{'Cycle':>10} | {'PC':>6} | {'VIA1 PB':>8} | {'PCR1':>4} | {'VIA2 PB':>8} | {'VIA2 PA':>8} | {'Sync'}")
    print("-" * 75)
    
    for i in range(512):
        pos = (current_index + i) % 512
        record_bytes = buffer_data[pos*16 : (pos+1)*16]
        
        # pub cycle: u32, pub pc: u32, pub event_type: u8, pub via1_pb: u8, pub via2_pb: u8,
        # pub pcr1: u8, pub pcr2: u8, pub via2_pa: u8, pub is_sync: u8, pub _padding: u8
        fields = struct.unpack("<IIBBBBBBBB", record_bytes)
        cycle, pc, event_type, via1_pb, via2_pb, pcr1, pcr2, via2_pa, is_sync, _ = fields
        
        if cycle == 0 and pc == 0:
            continue # Skip empty records if buffer isn't full yet
            
        sync_str = "SYNC" if is_sync else "    "
        via1_bin = f"{via1_pb:08b}"
        via2_bin = f"{via2_pb:08b}"
        via2_pa_hex = f"{via2_pa:02X}"
        pcr1_hex = f"{pcr1:02X}"
        
        print(f"{cycle:10d} | ${pc:04X} | {via1_bin} |  ${pcr1_hex} | {via2_bin} |      ${via2_pa_hex} | {sync_str}")

if __name__ == "__main__":
    main()
