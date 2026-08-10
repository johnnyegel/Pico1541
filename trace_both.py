import csv
import sys

with open("temp/Salae/Ar6-fastload-fail-3-retries.csv", "r") as f:
    reader = csv.reader(f)
    header = next(reader)
    col_t = header.index("Time [s]")
    cols = {
        "DataRx": header.index("DataRx"),
        "ClkRx": header.index("ClkRx"),
        "ATN": header.index("ATN"),
        "DataTx": header.index("DataTx"),
        "ClkTx": header.index("ClkTx"),
        "Debug0": header.index("Debug0")
    }
    
    last_t = 0
    last_tx_d = 1
    last_tx_c = 1
    last_d0 = 0
    
    for row in reader:
        t = float(row[col_t])
        if 2.74 < t < 2.80:
            tx_d = int(row[cols["DataTx"]])
            tx_c = int(row[cols["ClkTx"]])
            d0 = int(row[cols["Debug0"]])
            
            if tx_d != last_tx_d or tx_c != last_tx_c:
                delta = (t - last_t) * 1e6
                print(f"{t:.6f} (+{delta:.1f} us): Tx(D={tx_d}, C={tx_c})")
                last_t = t
                last_tx_d = tx_d
                last_tx_c = tx_c
            
            if d0 == 1 and last_d0 == 0:
                print(f"{t:.6f}: SO PULSE")
            last_d0 = d0
