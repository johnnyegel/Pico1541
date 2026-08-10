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
        if 2.7422 < t < 2.80:
            d = int(row[cols["DataTx"]])
            c = int(row[cols["ClkTx"]])
            events.append((t, 1-d, 1-c))
            
    clean = [events[0]]
    for e in events[1:]:
        if e[1] != clean[-1][1] or e[2] != clean[-1][2]:
            clean.append(e)

for i in range(1, 400):
    dur = (clean[i][0] - clean[i-1][0]) * 1e6
    if dur > 25.0:
        print(f"Gap: {dur:.1f} us at {clean[i-1][0]:.6f}")
