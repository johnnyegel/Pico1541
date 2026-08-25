import sys
import re

def decode_trace(input_file):
    # Regular expressions for parsing
    re_header = re.compile(r'#\d+ \(Trace\s+(load|store)\s+(dd00|1800)\)')
    re_detail = re.compile(r'\.(C|8):[0-9a-fA-F]+\s+(?:[0-9A-F]{2}\s*)+\s+(\w+).*?-\s+A:([0-9a-fA-F]{2})\s+X:([0-9a-fA-F]{2})\s+Y:([0-9a-fA-F]{2}).*?(\d+)$')

    events = []
    
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
                            val = int(a_val, 16)
                    elif action == 'load':
                        # For load, we can track state too
                        if opcode.startswith('LDA'):
                            val = int(a_val, 16)
                    
                    if val is not None:
                        events.append({
                            'time': timestamp,
                            'addr': addr,
                            'action': action,
                            'val': val
                        })
                current_header = None

    events.sort(key=lambda x: x['time'])

    if not events:
        print("No events found.")
        return

    first_time = events[0]['time']
    
    # State tracking
    c64_atn = 1
    c64_clk = 1
    c64_dat = 1
    d8_clk = 1
    d8_dat = 1
    d8_atna = 1
    
    print("Time (us) | C64: ATN CLK DAT | 1541: CLK DAT (ATNA) | Bus State (Physical)")
    print("-" * 75)
    
    for ev in events:
        t = ev['time'] - first_time
        val = ev['val']
        changed = False
        
        old_c_atn = c64_atn
        old_c_clk = c64_clk
        old_c_dat = c64_dat
        old_8_clk = d8_clk
        old_8_dat = d8_dat
        old_8_atna = d8_atna
        
        if ev['addr'] == 'dd00':
            c64_atn = 0 if ((val >> 3) & 1) else 1
            c64_clk = 0 if ((val >> 4) & 1) else 1
            c64_dat = 0 if ((val >> 5) & 1) else 1
            if (c64_atn != old_c_atn) or (c64_clk != old_c_clk) or (c64_dat != old_c_dat):
                changed = True
        elif ev['addr'] == '1800':
            d8_dat = 0 if ((val >> 1) & 1) else 1
            d8_clk = 0 if ((val >> 3) & 1) else 1
            d8_atna = 1 if ((val >> 4) & 1) else 0
            if (d8_clk != old_8_clk) or (d8_dat != old_8_dat) or (d8_atna != old_8_atna):
                changed = True
                
        if changed:
            # Physical bus (0 = Asserted, 1 = Released)
            phys_atn = c64_atn
            phys_clk = 0 if (c64_clk == 0 or d8_clk == 0) else 1
            phys_dat = 0 if (c64_dat == 0 or d8_dat == 0) else 1
            
            c_str = f"{'A' if c64_atn==0 else '-'} {'C' if c64_clk==0 else '-'} {'D' if c64_dat==0 else '-'}"
            d_str = f"{'C' if d8_clk==0 else '-'} {'D' if d8_dat==0 else '-'} ({'1' if d8_atna==1 else '0'})"
            p_str = f"ATN:{'A' if phys_atn==0 else 'R'} CLK:{'A' if phys_clk==0 else 'R'} DAT:{'A' if phys_dat==0 else 'R'}"
            
            print(f"{t:<9} | {c_str:<16} | {d_str:<20} | {p_str}")

if __name__ == '__main__':
    decode_trace(sys.argv[1])
