def track_sector_to_offset(t, s):
    offset = 0
    for i in range(1, t):
        if i <= 17: offset += 21
        elif i <= 24: offset += 19
        elif i <= 30: offset += 18
        else: offset += 17
    return (offset + s) * 256

with open("temp/Cauldron.d64", "rb") as f:
    d = f.read()
off = track_sector_to_offset(17, 0)
print(f"Data: {list(d[off:off+32])}")
