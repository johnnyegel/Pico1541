import csv
import sys

def analyze_logic(csv_file):
    with open(csv_file, 'r') as f:
        reader = csv.reader(f)
        headers = next(reader)
        idx = {h.strip(): i for i, h in enumerate(headers[1:])}
        
        last_clk = None
        last_data = None
        
        bits = []
        state = "IDLE"
        byte_vals = []
        
        for row in reader:
            if not row: continue
            time = float(row[0])
            vals = [int(v.strip()) for v in row[1:]]
            
            atn = vals[idx['ATN']]
            bus_clk = vals[idx['ClkRx']] | vals[idx['ClkTx']]
            bus_data = vals[idx['DataRx']] | vals[idx['DataTx']]
            
            if state == "IDLE":
                if atn == 1:
                    state = "WAIT_DATA_RELEASE"
                elif bus_clk == 1:
                    state = "WAIT_DATA_RELEASE"
                    
            elif state == "WAIT_DATA_RELEASE":
                if last_data == 1 and bus_data == 0:
                    state = "READ_BITS"
                    
            elif state == "READ_BITS":
                if last_clk == 1 and bus_clk == 0:
                    bits.append(bus_data)
                    if len(bits) == 8:
                        val = 0
                        for i, b in enumerate(bits):
                            logical_bit = 0 if b else 1
                            val |= (logical_bit << i)
                        char = chr(val) if 32 <= val <= 126 else '.'
                        byte_vals.append((time, atn, val, char))
                        bits = []
                        state = "WAIT_DATA_RELEASE" # Might be another byte right away
                        
            # If ATN is released, state is reset.
            # But wait, if ATN changes, we might need to reset.
            
            last_clk = bus_clk
            last_data = bus_data

        for t, a, v, c in byte_vals:
            print(f"Time: {t*1000:.3f} ms, ATN={a} -> Byte: 0x{v:02X} ({v}) '{c}'")

if __name__ == '__main__':
    analyze_logic(sys.argv[1])
