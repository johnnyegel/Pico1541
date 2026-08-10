import csv
import sys

with open("temp/Salae/Ar6-fastload-fail-3-retries.csv", "r") as f:
    reader = csv.reader(f)
    header = next(reader)
    col_t = header.index("Time [s]")
    cols = {
        "DataTx": header.index("DataTx"),
        "ClkTx": header.index("ClkTx")
    }
    
    events = []
    
    for row in reader:
        t = float(row[col_t])
        if 2.7422 < t < 2.7480: 
            d = int(row[cols["DataTx"]])
            c = int(row[cols["ClkTx"]])
            events.append((t, 1-d, 1-c)) # Invert to logical values (0 = released, 1 = asserted)
            
    # Remove duplicates
    clean = [events[0]]
    for e in events[1:]:
        if e[1] != clean[-1][1] or e[2] != clean[-1][2]:
            clean.append(e)
            
    # The first event is the start sync (1,1) -> (0,0) at ~2.742252
    
    # Let's print the sequence of states and the duration they were held
    for i in range(len(clean)-1):
        t1, d1, c1 = clean[i]
        t2, d2, c2 = clean[i+1]
        dur = (t2 - t1) * 1e6
        print(f"State: {d1}{c1} for {dur:4.1f} us")
