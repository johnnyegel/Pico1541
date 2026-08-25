import sys

def decode_iec(input_file):
    print("Decoding IEC bus transmission...")
    
    with open(input_file, 'r') as f:
        lines = f.readlines()
        
    start_idx = 0
    for i, line in enumerate(lines):
        if line.startswith("---"):
            start_idx = i + 1
            break
            
    events = []
    for line in lines[start_idx:]:
        line = line.strip()
        if not line: continue
        parts = line.split('|')
        if len(parts) < 4: continue
        
        t = int(parts[0].strip())
        phys = parts[3].strip()
        
        atn = 1 if 'ATN:R' in phys else 0
        clk = 1 if 'CLK:R' in phys else 0
        dat = 1 if 'DAT:R' in phys else 0
        events.append((t, atn, clk, dat))
        
    if not events:
        return
        
    # Standard IEC decoder
    # Talker asserts CLK (0), sets DAT, releases CLK (1). Listener reads on CLK release (0->1).
    # Wait, standard IEC: listener reads when CLK goes from 0 (Asserted) to 1 (Released).
    # Data is inverted on the bus: 1 (Released) = logical 1, 0 (Asserted) = logical 0.
    
    bytes_transferred = []
    current_byte = 0
    bit_count = 0
    in_transfer = False
    is_atn = False
    
    last_clk = events[0][2]
    
    for t, atn, clk, dat in events:
        # Detect CLK rising edge (0 -> 1)
        if last_clk == 0 and clk == 1:
            # Clock released by talker! Read data.
            # Wait, is the C64 talking or the 1541?
            # Standard IEC: talker releases CLK.
            bit = 1 if dat == 1 else 0
            current_byte |= (bit << bit_count)
            bit_count += 1
            
            if bit_count == 8:
                bytes_transferred.append((current_byte, atn == 0))
                current_byte = 0
                bit_count = 0
                
        last_clk = clk
        
    print(f"Decoded {len(bytes_transferred)} bytes.")
    
    # Group by ATN state
    current_atn = None
    group = []
    
    for b, atn in bytes_transferred:
        if atn != current_atn:
            if group:
                state = "COMMAND" if current_atn else "DATA"
                hex_str = " ".join([f"{x:02X}" for x in group])
                print(f"[{state}] {hex_str}")
            group = []
            current_atn = atn
        group.append(b)
        
    if group:
        state = "COMMAND" if current_atn else "DATA"
        hex_str = " ".join([f"{x:02X}" for x in group])
        print(f"[{state}] {hex_str}")

if __name__ == '__main__':
    decode_iec(sys.argv[1])
