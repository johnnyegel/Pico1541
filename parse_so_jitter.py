import csv
import sys

def parse_so_jitter(filename):
    with open(filename, 'r') as f:
        reader = csv.reader(f)
        header = next(reader)
        col_time = header.index('Time [s]')
        col_dbg0 = header.index('Debug0')
        
        last_t = None
        last_dbg0 = None
        
        max_delta = 0
        min_delta = 999
        
        for row in reader:
            t = float(row[col_time])
            dbg0 = row[col_dbg0]
            
            if dbg0 == '1' and last_dbg0 == '0':
                if last_t is not None:
                    delta_us = (t - last_t) * 1e6
                    if delta_us > max_delta:
                        max_delta = delta_us
                        print(f"New max SO interval: {delta_us:.3f} us at {t:.6f}s")
                    if delta_us < min_delta:
                        min_delta = delta_us
                last_t = t
                
            last_dbg0 = dbg0

parse_so_jitter("temp/Salae/Ar6-fastload-track-cache.csv")
