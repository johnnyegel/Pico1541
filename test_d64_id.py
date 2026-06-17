def check_d64_id():
    with open("temp/Cauldron.d64", "rb") as f:
        # Track 18 is track_index 17
        offset = 0
        for i in range(1, 18):
            if i <= 17: offset += 21 * 256
        f.seek(offset)
        sector0 = f.read(256)
        id0 = sector0[162]
        id1 = sector0[163]
        print(f"BAM ID: {chr(id0)}{chr(id1)} ({hex(id0)}, {hex(id1)})")

check_d64_id()
