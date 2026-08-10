import csv

with open("temp/Salae/Ar6-fastload-fail-3-retries.csv", "r") as f:
    reader = csv.reader(f)
    header = next(reader)
    col_t = header.index("Time [s]")
    cols = {
        "DataTx": header.index("DataTx"),
        "ClkTx": header.index("ClkTx")
    }
    
    last_d = 1
    bits = []
    
    for row in reader:
        t = float(row[col_t])
        if 2.7422 < t < 2.75:
            d = int(row[cols["DataTx"]])
            c = int(row[cols["ClkTx"]])
            
            # If C is clock, maybe we sample D on falling edge of C? (C goes 1 -> 0)
            if last_d == 1 and d == 0:
                bits.append(1 - c) # Inverted
                
            last_d = d

    # Print bytes
    for i in range(0, len(bits), 8):
        b = bits[i:i+8]
        if len(b) == 8:
            val = sum(b[j] << (7-j) for j in range(8))
            print(f"Byte: {val:02X}")
