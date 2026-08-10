import csv

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
        if 2.7422 < t < 2.743: # First 1000 us
            d = int(row[cols["DataTx"]])
            c = int(row[cols["ClkTx"]])
            events.append((t, 1-d, 1-c)) # Invert to logical values (0 = released, 1 = asserted)
            
    # Remove duplicates
    clean = [events[0]]
    for e in events[1:]:
        if e[1] != clean[-1][1] or e[2] != clean[-1][2]:
            clean.append(e)
            
    for e in clean:
        print(f"{(e[0] - clean[0][0])*1e6:7.1f} us: Data={e[1]} Clk={e[2]}")
