import sys

def unGCR(gcr):
    # standard decode table
    decode = {
        0b01010: 0x0, 0b01011: 0x1, 0b10010: 0x2, 0b10011: 0x3,
        0b01110: 0x4, 0b01111: 0x5, 0b10110: 0x6, 0b10111: 0x7,
        0b01001: 0x8, 0b11001: 0x9, 0b11010: 0xA, 0b11011: 0xB,
        0b01101: 0xC, 0b11101: 0xD, 0b11110: 0xE, 0b10101: 0xF
    }
    b1 = gcr[0]
    b2 = gcr[1]
    b3 = gcr[2]
    b4 = gcr[3]
    b5 = gcr[4]
    
    n1 = b1 >> 3
    n2 = ((b1 & 0x07) << 2) | (b2 >> 6)
    n3 = (b2 >> 1) & 0x1F
    n4 = ((b2 & 0x01) << 4) | (b3 >> 4)
    n5 = ((b3 & 0x0F) << 1) | (b4 >> 7)
    n6 = (b4 >> 2) & 0x1F
    n7 = ((b4 & 0x03) << 3) | (b5 >> 5)
    n8 = b5 & 0x1F
    
    try:
        out = bytearray(4)
        out[0] = (decode[n1] << 4) | decode[n2]
        out[1] = (decode[n3] << 4) | decode[n4]
        out[2] = (decode[n5] << 4) | decode[n6]
        out[3] = (decode[n7] << 4) | decode[n8]
        return out
    except KeyError:
        return None

def analyze_g64(filename, track_num):
    with open(filename, "rb") as f:
        data = f.read()
    
    if data[:8] != b"GCR-1541":
        print("Not a valid G64 file.")
        return
    
    tracks = data[9]
    track_offsets = []
    for i in range(tracks):
        off = int.from_bytes(data[12 + i*4:16 + i*4], "little")
        track_offsets.append(off)
        
    track_offset = track_offsets[track_num * 2 - 2]
    if track_offset == 0:
        print("Track empty")
        return
        
    length = int.from_bytes(data[track_offset:track_offset+2], "little")
    track_data = data[track_offset+2:track_offset+2+length]
    
    print(f"--- G64 Analysis of {filename} - Track {track_num} ---")
    print("Physical Sectors as they appear on the magnetic track:")
    
    sync_count = 0
    in_sync = False
    
    sectors_found = []
    
    i = 0
    while i < len(track_data):
        b = track_data[i]
        if b == 0xFF:
            if not in_sync:
                in_sync = True
                sync_count += 1
            i += 1
            continue
        
        if in_sync:
            in_sync = False
            # Look ahead for Header ID (0x08)
            gcr_bytes = track_data[i:i+5]
            if len(gcr_bytes) == 5:
                decoded = unGCR(gcr_bytes)
                if decoded and decoded[0] == 0x08:
                    sector = decoded[2]
                    track = decoded[3]
                    sectors_found.append(sector)
        i += 1
        
    print(f"Physical Layout: {sectors_found}")

if __name__ == "__main__":
    analyze_g64("temp/Cauldron.g64", 18)
    analyze_g64("temp/Cauldron.g64", 17)
