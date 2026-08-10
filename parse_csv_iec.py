import csv
import sys

def parse_csv(filename, start_time, end_time):
    with open(filename, 'r') as f:
        reader = csv.reader(f)
        header = next(reader)
        # Time [s],DataRx,SYNC,ClkRx,Debug1,ATN,Debug0,DataTx,ClkTx
        col_time = header.index('Time [s]')
        col_drx = header.index('DataRx')
        col_crx = header.index('ClkRx')
        col_atn = header.index('ATN')
        col_dtx = header.index('DataTx')
        col_ctx = header.index('ClkTx')
        col_dbg0 = header.index('Debug0')
        col_dbg1 = header.index('Debug1')
        
        last_state = None
        
        print(f"Time, DataRx, ClkRx, ATN, DataTx, ClkTx, Debug0, Debug1")
        
        count = 0
        for row in reader:
            t = float(row[col_time])
            if t < start_time:
                continue
            if t > end_time:
                break
                
            state = (row[col_drx], row[col_crx], row[col_atn], row[col_dtx], row[col_ctx], row[col_dbg0], row[col_dbg1])
            if state != last_state:
                print(f"{t:.6f}: {state}")
                last_state = state
                count += 1
                if count > 50:
                    break

parse_csv("temp/Salae/Ar6-fastload-track-cache.csv", 2.05, 2.15)
