import sys
import csv

def csv_to_vcd(input_csv, output_vcd):
    with open(input_csv, 'r') as f_in:
        reader = csv.reader(f_in)
        headers = next(reader)
        signal_names = headers[1:]
        
        atn_idx = -1
        for i, name in enumerate(signal_names):
            if name.strip().upper() == "ATN":
                atn_idx = i
                break
                
        rows = list(reader)
        
    if not rows: return
    
    def parse_time(time_str):
        return int(float(time_str) * 1_000_000_000)
        
    sync_time_raw = None
    if atn_idx != -1:
        for row in rows:
            if not row: continue
            if row[atn_idx + 1].strip() == '1':
                sync_time_raw = parse_time(row[0])
                break
                
    if sync_time_raw is None:
        sync_time_raw = parse_time(rows[0][0])
        
    # We want sync_time_raw to map to 100,000,000 ns (100ms)
    vcd_time_offset = sync_time_raw - 100_000_000

    with open(output_vcd, 'w') as f_out:
        f_out.write("$date Today $end\n")
        f_out.write("$timescale 1ns $end\n")
        f_out.write("$scope module Hardware_Capture $end\n")
        
        sig_ids = {}
        for i, name in enumerate(signal_names):
            ident = chr(33 + i)
            sig_name = f"hw_{name.strip()}"
            sig_ids[i] = ident
            f_out.write(f"$var wire 1 {ident} {sig_name} $end\n")
            
        f_out.write("$upscope $end\n")
        f_out.write("$enddefinitions $end\n")
        
        f_out.write("$dumpvars\n")
        prev_vals = [val.strip() for val in rows[0][1:]]
        for i, val in enumerate(prev_vals):
            f_out.write(f"{val}{sig_ids[i]}\n")
        f_out.write("$end\n")
        
        last_time_output = -1
        
        for row in rows:
            if not row: continue
            t = parse_time(row[0]) - vcd_time_offset
            
            # Skip rows before t=0
            if t < 0:
                # Still update prev_vals silently
                for i, val in enumerate(row[1:]):
                    prev_vals[i] = val.strip()
                continue
                
            changes = []
            for i, val in enumerate(row[1:]):
                val = val.strip()
                if val != prev_vals[i]:
                    changes.append((i, val))
                    prev_vals[i] = val
                    
            # Always output first valid time event even if no changes (usually time 0)
            if changes or last_time_output == -1:
                if t > last_time_output:
                    f_out.write(f"#{t}\n")
                    last_time_output = t
                for i, val in changes:
                    f_out.write(f"{val}{sig_ids[i]}\n")

if __name__ == '__main__':
    csv_to_vcd(sys.argv[1], sys.argv[2])
