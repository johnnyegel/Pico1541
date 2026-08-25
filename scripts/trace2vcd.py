import sys
import re

def parse_trace(input_file, output_file):
    events = []
    
    # Regular expressions for parsing
    # Trace headers
    re_header = re.compile(r'#\d+ \(Trace\s+(load|store)\s+(dd00|1800)\)')
    # Trace details: .C:0015  91 0E       STA ($0E),Y    - A:70 X:FF Y:00 SP:f6 ..-..I.C 78139996250
    # .8:e9b7  AD 00 18    LDA $1800      - A:85 X:45 Y:FF SP:43 N.-..I.C 78147183811
    re_detail = re.compile(r'\.(C|8):[0-9a-fA-F]+\s+(?:[0-9A-F]{2}\s*)+\s+(\w+).*?-\s+A:([0-9a-fA-F]{2})\s+X:([0-9a-fA-F]{2})\s+Y:([0-9a-fA-F]{2}).*?(\d+)$')

    with open(input_file, 'r', encoding='latin-1') as f:
        current_header = None
        for line in f:
            line = line.strip()
            
            m_header = re_header.search(line)
            if m_header:
                current_header = m_header.groups()
                continue
                
            if current_header and line.startswith('.'):
                m_detail = re_detail.search(line)
                if m_detail:
                    action, addr = current_header
                    cpu, opcode, a_val, x_val, y_val, timestamp = m_detail.groups()
                    timestamp = int(timestamp)
                    
                    val = None
                    if action == 'store':
                        if opcode.startswith('STA'):
                            val = int(a_val, 16)
                        elif opcode.startswith('STX'):
                            val = int(x_val, 16)
                        elif opcode.startswith('STY'):
                            val = int(y_val, 16)
                        else:
                            val = int(a_val, 16) # Fallback
                    elif action == 'load':
                        if opcode.startswith('LDA'):
                            val = int(a_val, 16)
                        elif opcode.startswith('LDX'):
                            val = int(x_val, 16)
                        elif opcode.startswith('LDY'):
                            val = int(y_val, 16)
                        else:
                            val = int(a_val, 16) # Fallback
                    
                    if val is not None:
                        events.append({
                            'time': timestamp,
                            'addr': addr,
                            'action': action,
                            'val': val
                        })
                current_header = None

    # Sort events by timestamp
    events.sort(key=lambda x: x['time'])

    # Write VCD
    with open(output_file, 'w') as f:
        f.write("$date Today $end\n")
        f.write("$timescale 1us $end\n")
        f.write("$scope module IEC $end\n")
        
        signals = {
            'c64_dd00': {'id': 'C', 'val': 0},
            'c64_atn_out': {'id': 'C3', 'val': 0},
            'c64_clk_out': {'id': 'C4', 'val': 0},
            'c64_data_out': {'id': 'C5', 'val': 0},
            'c64_clk_in': {'id': 'C6', 'val': 0},
            'c64_data_in': {'id': 'C7', 'val': 0},
            
            'd8_1800': {'id': 'D', 'val': 0},
            'd8_data_in': {'id': 'D0', 'val': 0},
            'd8_data_out': {'id': 'D1', 'val': 0},
            'd8_clk_in': {'id': 'D2', 'val': 0},
            'd8_clk_out': {'id': 'D3', 'val': 0},
            'd8_atna': {'id': 'D4', 'val': 0},
            'd8_atn_in': {'id': 'D7', 'val': 0},
        }
        
        # 8-bit bus signals
        f.write(f"$var wire 8 {signals['c64_dd00']['id']} c64_dd00_hex $end\n")
        f.write(f"$var wire 8 {signals['d8_1800']['id']} d8_1800_hex $end\n")
        
        # Bits C64
        f.write(f"$var wire 1 {signals['c64_atn_out']['id']} c64_atn_out $end\n")
        f.write(f"$var wire 1 {signals['c64_clk_out']['id']} c64_clk_out $end\n")
        f.write(f"$var wire 1 {signals['c64_data_out']['id']} c64_data_out $end\n")
        f.write(f"$var wire 1 {signals['c64_clk_in']['id']} c64_clk_in $end\n")
        f.write(f"$var wire 1 {signals['c64_data_in']['id']} c64_data_in $end\n")
        
        # Bits 1541
        f.write(f"$var wire 1 {signals['d8_data_in']['id']} d8_data_in $end\n")
        f.write(f"$var wire 1 {signals['d8_data_out']['id']} d8_data_out $end\n")
        f.write(f"$var wire 1 {signals['d8_clk_in']['id']} d8_clk_in $end\n")
        f.write(f"$var wire 1 {signals['d8_clk_out']['id']} d8_clk_out $end\n")
        f.write(f"$var wire 1 {signals['d8_atna']['id']} d8_atna $end\n")
        f.write(f"$var wire 1 {signals['d8_atn_in']['id']} d8_atn_in $end\n")
        
        f.write("$upscope $end\n")
        f.write("$enddefinitions $end\n")
        
        # Initial state (dumpvars)
        f.write("$dumpvars\n")
        f.write(f"b00000000 {signals['c64_dd00']['id']}\n")
        f.write(f"b00000000 {signals['d8_1800']['id']}\n")
        for k, v in signals.items():
            if k not in ['c64_dd00', 'd8_1800']:
                f.write(f"0{v['id']}\n")
        f.write("$end\n")
        
        if not events:
            return
            
        first_time = events[0]['time']
        last_time = -1
        
        for ev in events:
            t = ev['time'] - first_time
            if t > last_time:
                f.write(f"#{t}\n")
                last_time = t
                
            val = ev['val']
            if ev['addr'] == 'dd00':
                f.write(f"b{val:08b} {signals['c64_dd00']['id']}\n")
                # c64 atn out (bit 3)
                f.write(f"{(val >> 3) & 1}{signals['c64_atn_out']['id']}\n")
                f.write(f"{(val >> 4) & 1}{signals['c64_clk_out']['id']}\n")
                f.write(f"{(val >> 5) & 1}{signals['c64_data_out']['id']}\n")
                f.write(f"{(val >> 6) & 1}{signals['c64_clk_in']['id']}\n")
                f.write(f"{(val >> 7) & 1}{signals['c64_data_in']['id']}\n")
            elif ev['addr'] == '1800':
                f.write(f"b{val:08b} {signals['d8_1800']['id']}\n")
                f.write(f"{(val >> 0) & 1}{signals['d8_data_in']['id']}\n")
                f.write(f"{(val >> 1) & 1}{signals['d8_data_out']['id']}\n")
                f.write(f"{(val >> 2) & 1}{signals['d8_clk_in']['id']}\n")
                f.write(f"{(val >> 3) & 1}{signals['d8_clk_out']['id']}\n")
                f.write(f"{(val >> 4) & 1}{signals['d8_atna']['id']}\n")
                f.write(f"{(val >> 7) & 1}{signals['d8_atn_in']['id']}\n")

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print("Usage: python trace2vcd.py <input.txt> <output.vcd>")
        sys.exit(1)
    parse_trace(sys.argv[1], sys.argv[2])
