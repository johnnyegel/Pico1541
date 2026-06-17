import sys

def track_sector_to_offset(t, s):
    offset = 0
    for i in range(1, t):
        if i <= 17: offset += 21
        elif i <= 24: offset += 19
        elif i <= 30: offset += 18
        else: offset += 17
    return (offset + s) * 256

def analyze_d64(filename, track):
    with open(filename, "rb") as f:
        data = f.read()
    
    sectors = 21 if track <= 17 else 19 if track <= 24 else 18 if track <= 30 else 17
    
    print(f"--- D64 Analysis of {filename} - Track {track} ---")
    print("Sequential Logical Sectors as they appear in the file:")
    for s in range(sectors):
        off = track_sector_to_offset(track, s)
        next_t = data[off]
        next_s = data[off+1]
        print(f"  File offset {off:06X} -> Logical Sector {s:02d} | Next Pointer: T={next_t:02d}, S={next_s:02d}")

if __name__ == "__main__":
    analyze_d64("temp/Cauldron.d64", 18)
    print()
    analyze_d64("temp/Cauldron.d64", 17)
    analyze_d64("temp/Cauldron.d64", 16)
    print()
    analyze_d64("temp/Cauldron.d64", 15)
