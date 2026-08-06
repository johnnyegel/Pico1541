import json
import base64
import sys

def load_lac(filename):
    with open(filename, 'r') as f:
        data = json.load(f)
    freq = data['Settings']['Frequency']
    channels = {c['ChannelName']: list(base64.b64decode(c['Samples'])) for c in data['Settings']['CaptureChannels']}
    return freq, channels

def c64_read_byte(samples, clk_ch, data_ch, base_c64_cycle):
    reads = []
    val = 0
    
    # 4 reads at +0, +8, +16, +24 cycles from base
    for offset in [0, 8, 16, 24]:
        cycle = base_c64_cycle + offset
        sample_idx = int(cycle * 2.0)
        if sample_idx >= len(clk_ch):
            return None, None
            
        c = clk_ch[sample_idx]
        d = data_ch[sample_idx]
        
        # c=1 means asserted, which is 0V on bus, so C64 reads 0
        c64_c = 0 if c == 1 else 1
        c64_d = 0 if d == 1 else 1
        
        reads.append((cycle, c, d, c64_c, c64_d))
        
    if len(reads) < 4:
        return None, None
        
    # Emulate the C64 instruction sequence
    # lda $DD00
    a = (reads[0][4] << 7) | (reads[0][3] << 6)
    
    # lsr a, lsr a, eor $DD00
    a = (a >> 2) ^ ((reads[1][4] << 7) | (reads[1][3] << 6))
    
    # lsr a, lsr a, eor $DD00
    a = (a >> 2) ^ ((reads[2][4] << 7) | (reads[2][3] << 6))
    
    # lsr a, lsr a, eor $DD00
    a = (a >> 2) ^ ((reads[3][4] << 7) | (reads[3][3] << 6))
    
    return a, reads

def analyze(filename):
    freq, channels = load_lac(filename)
    clk = channels['ClkTx']
    data_tx = channels['DataTx']
    data_rx = channels['DataRX']
    
    start_idx = -1
    for i in range(1, len(data_rx)):
        if data_rx[i] == 0 and data_rx[i-1] == 1:
            start_idx = i
            break
            
    if start_idx == -1: return
    
    c64_cycle = start_idx / 2.0
    c64_cycle += 49
    
    print("Decoded bytes:")
    for block in range(20):
        block_bytes = []
        for byte_idx in range(4):
            base = c64_cycle + byte_idx * 31
            val, reads = c64_read_byte(channels, clk, data_tx, base)
            if val is None: break
            block_bytes.append(val)
        
        if len(block_bytes) == 4:
            b0, b1, b2, b3 = block_bytes
            print(f"Block {block:2d}: {b0:02X} {b1:02X} {b2:02X} {b3:02X}  (XOR F9: {b0^0xF9:02X} {b1^0xF9:02X} {b2^0xF9:02X} {b3^0xF9:02X})")
            
        c64_cycle += 187

if __name__ == "__main__":
    analyze('temp/LogicCaptures/strange-pulses.lac')
