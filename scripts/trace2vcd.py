import sys
import re

def parse_trace(input_file, output_file):
    events = []
    re_header = re.compile(r'#\d+ \(Trace\s+(load|store)\s+(dd00|1800)\)')
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
                        events.append({
                            'time': timestamp,
                            'addr': addr,
                            'action': action,
                            'val': val
                        })
                current_header = None
    events.sort(key=lambda x: x['time'])
    
    if not events: return
    
    # Find sync time (first ATN assert)
    # C64 ATN out is (val >> 3) & 1.
    sync_time_raw = None
    state_c64 = 0
    for ev in events:
        if ev['addr'] == 'dd00':
            state_c64 = ev['val']
            atn_asserted = (state_c64 >> 3) & 1
            if atn_asserted:
                sync_time_raw = ev['time']
                break
                
    if sync_time_raw is None:
        sync_time_raw = events[0]['time']

    # Keep only events from up to 100ms before sync
    time_offset = sync_time_raw - 100_000
    events = [ev for ev in events if ev['time'] >= time_offset]
    if not events: return
    first_time = events[0]['time']
    
    # We want sync_time_raw to map to 100,000 (100ms) in VCD
    vcd_time_offset = sync_time_raw - 100_000

    with open(output_file, 'w') as f:
        f.write("$date Today $end\n")
        f.write("$timescale 1us $end\n")
        f.write("$scope module VICE_Trace $end\n")
        signals = {
            'c64_dd00': {'id': 'C', 'val': 0},
            'd8_1800': {'id': 'D', 'val': 0},
            'pico_atn_rx': {'id': 'P0', 'val': 0},
            'pico_clk_rx': {'id': 'P1', 'val': 0},
            'pico_data_rx': {'id': 'P2', 'val': 0},
            'pico_clk_tx': {'id': 'P3', 'val': 0},
            'pico_data_tx': {'id': 'P4', 'val': 0},
            'bus_atn': {'id': 'B0', 'val': 1},
            'bus_clk': {'id': 'B1', 'val': 1},
            'bus_data': {'id': 'B2', 'val': 1},
        }
        f.write(f"$var wire 8 {signals['c64_dd00']['id']} c64_dd00_hex $end\n")
        f.write(f"$var wire 8 {signals['d8_1800']['id']} d8_1800_hex $end\n")
        f.write(f"$var wire 1 {signals['pico_atn_rx']['id']} pico_atn_rx $end\n")
        f.write(f"$var wire 1 {signals['pico_clk_rx']['id']} pico_clk_rx $end\n")
        f.write(f"$var wire 1 {signals['pico_data_rx']['id']} pico_data_rx $end\n")
        f.write(f"$var wire 1 {signals['pico_clk_tx']['id']} pico_clk_tx $end\n")
        f.write(f"$var wire 1 {signals['pico_data_tx']['id']} pico_data_tx $end\n")
        f.write(f"$var wire 1 {signals['bus_atn']['id']} bus_atn $end\n")
        f.write(f"$var wire 1 {signals['bus_clk']['id']} bus_clk $end\n")
        f.write(f"$var wire 1 {signals['bus_data']['id']} bus_data $end\n")
        f.write("$upscope $end\n")
        f.write("$enddefinitions $end\n")
        
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
            atn_line = 0 if atn_asserted else 1
            clk_line = 0 if clk_asserted else 1
            data_line = 0 if data_asserted else 1
            f.write(f"{atn_line}{signals['bus_atn']['id']}\n")
            f.write(f"{clk_line}{signals['bus_clk']['id']}\n")
            f.write(f"{data_line}{signals['bus_data']['id']}\n")
            f.write(f"{atn_asserted}{signals['pico_atn_rx']['id']}\n")
            f.write(f"{clk_asserted}{signals['pico_clk_rx']['id']}\n")
            f.write(f"{data_asserted}{signals['pico_data_rx']['id']}\n")
            f.write(f"{d8_clk}{signals['pico_clk_tx']['id']}\n")
            f.write(f"{d8_data}{signals['pico_data_tx']['id']}\n")

        f.write("$dumpvars\n")
        f.write(f"b00000000 {signals['c64_dd00']['id']}\n")
        f.write(f"b00000000 {signals['d8_1800']['id']}\n")
        write_bus_state()
        f.write("$end\n")
        
        last_time = -1
        for ev in events:
            t = ev['time'] - vcd_time_offset
            if t > last_time:
                f.write(f"#{t}\n")
                last_time = t
            val = ev['val']
            if ev['addr'] == 'dd00':
                state_c64_dd00 = val
                f.write(f"b{val:08b} {signals['c64_dd00']['id']}\n")
                write_bus_state()
            elif ev['addr'] == '1800':
                state_d8_1800 = val
                f.write(f"b{val:08b} {signals['d8_1800']['id']}\n")
                write_bus_state()

if __name__ == '__main__':
    parse_trace(sys.argv[1], sys.argv[2])
