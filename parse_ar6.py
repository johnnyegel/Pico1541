import csv
import sys

def parse_ar6(filename, start_time, end_time):
    with open(filename, 'r') as f:
        reader = csv.reader(f)
        header = next(reader)
        col_time = header.index('Time [s]')
        col_drx = header.index('DataRx')
        col_crx = header.index('ClkRx')
        col_atn = header.index('ATN')
        col_dtx = header.index('DataTx')
        col_ctx = header.index('ClkTx')
        
        last_state = None
        
        for row in reader:
            t = float(row[col_time])
            if t < start_time:
                continue
            if t > end_time:
                break
                
            state = (row[col_drx], row[col_crx], row[col_atn], row[col_dtx], row[col_ctx])
            if state != last_state:
                if last_state is not None:
                    changed = [i for i in range(5) if state[i] != last_state[i]]
                    names = ['DataRx', 'ClkRx', 'ATN', 'DataTx', 'ClkTx']
                    changes = [f"{names[i]}->{state[i]}" for i in changed]
                    print(f"{t:.6f}: {', '.join(changes)}")
                last_state = state

parse_ar6("temp/Salae/Ar6-fastload-track-cache.csv", 2.05, 2.055)
