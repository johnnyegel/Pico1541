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

def get_state(t):
    state = (1,1)
    for e in clean:
        if e[0] <= t:
            state = (e[1], e[2])
        else:
            break
    return state

# Find first sync (1,1) -> (0,0)
sync_t = None
for i in range(len(clean)-1):
    if clean[i][1:] == (1,1) and clean[i+1][1:] == (0,0):
        sync_t = clean[i+1][0]
        break

t = sync_t
# Wait for release of sync (0,0) -> (1,1)
while get_state(t) == (0,0):
    t += 1e-6

# C64 waits a few cycles after release to start its loop
# Let's dynamically find the first write by looking for the first state change after release
while get_state(t) == (1,1):
    t += 1e-6

# t is now exactly at Write 1 of Nibble 1
# C64 samples at t + 4us and t + 12us
nibbles = []
for _ in range(512): # Read 256 bytes (512 nibbles)
    p1 = get_state(t + 4e-6)
    p2 = get_state(t + 12e-6)
    
    n = (p1[0] << 3) | (p1[1] << 2) | (p2[0] << 1) | (p2[1])
    nibbles.append(n)
    
    t += 28e-6 # Advance to next nibble (8 + 20)

for i in range(0, len(nibbles), 2):
    if i+1 < len(nibbles):
        b = (nibbles[i] << 4) | nibbles[i+1]
        print(f"{b:02X}", end=" ")
        if (i//2 + 1) % 16 == 0:
            print()
