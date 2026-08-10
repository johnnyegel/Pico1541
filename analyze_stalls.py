import csv
import sys
from collections import defaultdict

def analyze_csv(filename):
    print(f"Analyzing {filename}...")
    
    last_debug0_time = None
    last_debug1_time = None
    last_sync = None
    
    debug0_intervals = []
    debug1_intervals = []
    
    anomalies_debug0 = []
    anomalies_debug1 = []
    
    line_count = 0
    sync_0_start = None
    
    with open(filename, 'r') as f:
        reader = csv.reader(f)
        headers = next(reader)
        
        # Headers: Time [s],DataRx,SYNC,ClkRx,Debug1,ATN,Debug0,DataTx,ClkTx
        col_time = headers.index('Time [s]')
        col_sync = headers.index('SYNC')
        col_debug0 = headers.index('Debug0')
        col_debug1 = headers.index('Debug1')
        
        prev_debug0 = 0
        prev_debug1 = 0
        
        for row in reader:
            line_count += 1
            if line_count % 1000000 == 0:
                print(f"Processed {line_count} lines...")
                
            t = float(row[col_time])
            sync = int(row[col_sync])
            debug0 = int(row[col_debug0])
            debug1 = int(row[col_debug1])
            
            # Detect SYNC going LOW (data mode starts)
            if sync == 0 and last_sync == 1:
                sync_0_start = t
                last_debug0_time = None # Reset interval measurement on new sector
            
            # Detect Debug0 (SO pulse) rising edge
            if debug0 == 1 and prev_debug0 == 0:
                if sync == 0: # Only care about SO pulses during data mode
                    if last_debug0_time is not None:
                        delta_us = (t - last_debug0_time) * 1_000_000
                        debug0_intervals.append(delta_us)
                        # Speed zones: 1 byte = 25us, 27us, 29us, or 32us.
                        # If delta is > 40us, we missed a byte!
                        if delta_us > 40.0 and delta_us < 200.0:
                            anomalies_debug0.append((t, delta_us))
                    last_debug0_time = t
            
            # Detect Debug1 (IEC access) rising edge
            if debug1 == 1 and prev_debug1 == 0:
                if last_debug1_time is not None:
                    delta_us = (t - last_debug1_time) * 1_000_000
                    debug1_intervals.append(delta_us)
                    # Arbitrary threshold: if > 1000us, CPU might be stalled or just waiting.
                    # We'll just collect stats for now.
                    if delta_us > 5000.0:
                        pass # Ignore long waits, probably track changes or idle
                    elif delta_us > 100.0:
                        anomalies_debug1.append((t, delta_us))
                last_debug1_time = t
                
            prev_debug0 = debug0
            prev_debug1 = debug1
            last_sync = sync
            
    print("\n--- ANALYSIS RESULTS ---")
    print(f"Total lines processed: {line_count}")
    
    if debug0_intervals:
        print("\nDebug0 (SO Pulses during Data Mode) Stats:")
        print(f"Count: {len(debug0_intervals)}")
        print(f"Min interval: {min(debug0_intervals):.2f} us")
        print(f"Max interval: {max(debug0_intervals):.2f} us")
        print(f"Avg interval: {sum(debug0_intervals)/len(debug0_intervals):.2f} us")
        
        print(f"Found {len(anomalies_debug0)} anomalies (>40us):")
        for t, delta in anomalies_debug0[:20]:
            print(f"  At {t:.6f}s : Gap of {delta:.2f} us")
        if len(anomalies_debug0) > 20:
            print(f"  ... and {len(anomalies_debug0) - 20} more.")
    else:
        print("\nNo Debug0 pulses found during data mode!")

    if debug1_intervals:
        print("\nDebug1 (IEC Access) Stats (excluding >5ms gaps):")
        filtered_debug1 = [x for x in debug1_intervals if x <= 5000.0]
        if filtered_debug1:
            print(f"Count: {len(filtered_debug1)}")
            print(f"Min interval: {min(filtered_debug1):.2f} us")
            print(f"Max interval: {max(filtered_debug1):.2f} us")
            print(f"Avg interval: {sum(filtered_debug1)/len(filtered_debug1):.2f} us")
            print(f"Found {len(anomalies_debug1)} delays between 100us and 5000us.")
            # Group anomalies by length roughly
            hist = defaultdict(int)
            for _, d in anomalies_debug1:
                hist[int(d/100)*100] += 1
            print("  Distribution of these delays (bucketed by 100us):")
            for k in sorted(hist.keys()):
                print(f"    {k} - {k+99} us : {hist[k]} occurrences")
        
if __name__ == '__main__':
    analyze_csv('temp/Salae/Ar6-fastload-short-stalls.csv')
