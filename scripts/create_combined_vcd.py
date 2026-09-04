import sys
import csv
import re

def create_combined(trace_file, csv_file, out_file):
    # --- Parse VICE Trace ---
    events_vice = []
    re_header = re.compile(r'#\d+ \(Trace\s+(load|store)\s+(dd00|1800)\)')
    re_detail = re.compile(r'\.(C|8):[0-9a-fA-F]+\s+(?:[0-9A-F]{2}\s*)+\s+(\w+).*?-\s+A:([0-9a-fA-F]{2})\s+X:([0-9a-fA-F]{2})\s+Y:([0-9a-fA-F]{2}).*?(\d+)$')

    with open(trace_file, 'r', encoding='latin-1') as f:
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
                    timestamp = int(timestamp) * 1000 # convert us to ns
                    val = None
                    if action == 'store':
                        if opcode.startswith('STA'): val = int(a_val, 16)
                        elif opcode.startswith('STX'): val = int(x_val, 16)
                        elif opcode.startswith('STY'): val = int(y_val, 16)
                        else: val = int(a_val, 16)
                    elif action == 'load':
                        if opcode.startswith('LDA'): val = int(a_val, 16)
                        elif opcode.startswith('LDX'): val = int(x_val, 16)
                        elif opcode.startswith('LDY'): val = int(y_val, 16)
                        else: val = int(a_val, 16)
                    if val is not None:
                        events_vice.append({'time': timestamp, 'addr': addr, 'val': val})
                current_header = None
    events_vice.sort(key=lambda x: x['time'])
    
    sync_time_vice = None
    state_c64 = 0
    for ev in events_vice:
        if ev['addr'] == 'dd00':
            state_c64 = ev['val']
            if (state_c64 >> 3) & 1:
                sync_time_vice = ev['time']
                break
    if sync_time_vice is None: sync_time_vice = events_vice[0]['time']
    
    # filter vice events
    events_vice = [ev for ev in events_vice if ev['time'] >= sync_time_vice - 100_000_000]
    
    # --- Parse Logic CSV ---
    events_logic = []
    with open(csv_file, 'r') as f:
        reader = csv.reader(f)
        headers = next(reader)
        signal_names = headers[1:]
        
        atn_idx = -1
        for i, name in enumerate(signal_names):
            if name.strip().upper() == "ATN":
                atn_idx = i
                break
        rows = list(reader)
        
    def parse_time(time_str):
        return int(float(time_str) * 1_000_000_000)
        
    sync_time_logic = None
    if atn_idx != -1:
        for row in rows:
            if not row: continue
            if row[atn_idx + 1].strip() == '1':
                sync_time_logic = parse_time(row[0])
                break
    if sync_time_logic is None: sync_time_logic = parse_time(rows[0][0])
    
    for row in rows:
        if not row: continue
        t = parse_time(row[0])
        if t >= sync_time_logic - 100_000_000:
            events_logic.append({'time': t, 'vals': [v.strip() for v in row[1:]]})
            
    # --- Merge ---
    with open(out_file, 'w') as f:
        f.write("$date Today $end\n")
        f.write("$timescale 1ns $end\n")
        
        f.write("$scope module VICE_Trace $end\n")
        sig_vice = {
            'c64_dd00': 'C', 'd8_1800': 'D',
            'pico_atn_rx': 'P0', 'pico_clk_rx': 'P1', 'pico_data_rx': 'P2',
            'pico_clk_tx': 'P3', 'pico_data_tx': 'P4',
            'bus_atn': 'B0', 'bus_clk': 'B1', 'bus_data': 'B2'
        }
        f.write(f"$var wire 8 {sig_vice['c64_dd00']} c64_dd00_hex $end\n")
        f.write(f"$var wire 8 {sig_vice['d8_1800']} d8_1800_hex $end\n")
        for k, v in sig_vice.items():
            if k not in ['c64_dd00', 'd8_1800']:
                f.write(f"$var wire 1 {v} {k} $end\n")
        f.write("$upscope $end\n")
        
        f.write("$scope module Hardware_Capture $end\n")
        sig_logic = {}
        for i, name in enumerate(signal_names):
            ident = chr(33 + i) # start at !
            # avoid conflicts with vice ids if possible.
            # vice uses C, D, P0-4, B0-2. chr(33) is !, ", #, $, %, etc.
            # perfectly safe.
            sig_logic[i] = ident
            f.write(f"$var wire 1 {ident} hw_{name.strip()} $end\n")
        f.write("$upscope $end\n")
        f.write("$enddefinitions $end\n")
        
        f.write("$dumpvars\n")
        f.write(f"b00000000 {sig_vice['c64_dd00']}\n")
        f.write(f"b00000000 {sig_vice['d8_1800']}\n")
        f.write(f"1{sig_vice['bus_atn']}\n1{sig_vice['bus_clk']}\n1{sig_vice['bus_data']}\n")
        f.write(f"0{sig_vice['pico_atn_rx']}\n0{sig_vice['pico_clk_rx']}\n0{sig_vice['pico_data_rx']}\n")
        f.write(f"0{sig_vice['pico_clk_tx']}\n0{sig_vice['pico_data_tx']}\n")
        
        state_logic = events_logic[0]['vals'] if events_logic else ['0']*len(signal_names)
        for i, val in enumerate(state_logic):
            f.write(f"{val}{sig_logic[i]}\n")
        f.write("$end\n")
        
        state_c64_dd00 = 0
        state_d8_1800 = 0
        def write_bus_state():
            c64_atn = (state_c64_dd00 >> 3) & 1
            c64_clk = (state_c64_dd00 >> 4) & 1
            c64_data = (state_c64_dd00 >> 5) & 1
            d8_data = (state_d8_1800 >> 1) & 1
            d8_clk = (state_d8_1800 >> 3) & 1
            d8_atna = (state_d8_1800 >> 4) & 1
            atn_asserted = c64_atn
            clk_asserted = c64_clk | d8_clk
            data_asserted = c64_data | d8_data | (atn_asserted & d8_atna)
            f.write(f"{0 if atn_asserted else 1}{sig_vice['bus_atn']}\n")
            f.write(f"{0 if clk_asserted else 1}{sig_vice['bus_clk']}\n")
            f.write(f"{0 if data_asserted else 1}{sig_vice['bus_data']}\n")
            f.write(f"{atn_asserted}{sig_vice['pico_atn_rx']}\n")
            f.write(f"{clk_asserted}{sig_vice['pico_clk_rx']}\n")
            f.write(f"{data_asserted}{sig_vice['pico_data_rx']}\n")
            f.write(f"{d8_clk}{sig_vice['pico_clk_tx']}\n")
            f.write(f"{d8_data}{sig_vice['pico_data_tx']}\n")
            
        # Combine events
        all_events = []
        for ev in events_vice:
            t = ev['time'] - sync_time_vice + 100_000_000
            all_events.append({'t': t, 'type': 'vice', 'data': ev})
        for ev in events_logic:
            t = ev['time'] - sync_time_logic + 100_000_000
            all_events.append({'t': t, 'type': 'logic', 'data': ev})
            
        all_events.sort(key=lambda x: x['t'])
        
        last_time = -1
        for ev in all_events:
            t = ev['t']
            if t < 0: continue
            
            if t > last_time:
                if last_time != -1 or t == 0:
                    f.write(f"#{t}\n")
                last_time = t
                
            if ev['type'] == 'vice':
                val = ev['data']['val']
                if ev['data']['addr'] == 'dd00':
                    state_c64_dd00 = val
                    f.write(f"b{val:08b} {sig_vice['c64_dd00']}\n")
                    write_bus_state()
                else:
                    state_d8_1800 = val
                    f.write(f"b{val:08b} {sig_vice['d8_1800']}\n")
                    write_bus_state()
            else:
                new_state = ev['data']['vals']
                for i, val in enumerate(new_state):
                    if val != state_logic[i]:
                        f.write(f"{val}{sig_logic[i]}\n")
                state_logic = new_state

if __name__ == '__main__':
    create_combined(sys.argv[1], sys.argv[2], sys.argv[3])
