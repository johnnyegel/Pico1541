import sys

# GCR decode table (5-bit to 4-bit)
gcr_to_nibble = {
    0x0A: 0, 0x0B: 1, 0x12: 2, 0x13: 3, 0x0E: 4, 0x0F: 5, 0x16: 6, 0x17: 7,
    0x09: 8, 0x19: 9, 0x1A: 10, 0x1B: 11, 0x0D: 12, 0x1D: 13, 0x1E: 14, 0x15: 15
}

def decode_5_bytes(data):
    b0, b1, b2, b3, b4 = data
    n0 = gcr_to_nibble.get(b0 >> 3, -1)
    n1 = gcr_to_nibble.get(((b0 & 0x07) << 2) | (b1 >> 6), -1)
    n2 = gcr_to_nibble.get((b1 >> 1) & 0x1F, -1)
    n3 = gcr_to_nibble.get(((b1 & 0x01) << 4) | (b2 >> 4), -1)
    n4 = gcr_to_nibble.get(((b2 & 0x0F) << 1) | (b3 >> 7), -1)
    n5 = gcr_to_nibble.get((b3 >> 2) & 0x1F, -1)
    n6 = gcr_to_nibble.get(((b3 & 0x03) << 3) | (b4 >> 5), -1)
    n7 = gcr_to_nibble.get(b4 & 0x1F, -1)
    
    if -1 in (n0, n1, n2, n3, n4, n5, n6, n7):
        return None
        
    out0 = (n0 << 4) | n1
    out1 = (n2 << 4) | n3
    out2 = (n4 << 4) | n5
    out3 = (n6 << 4) | n7
    return [out0, out1, out2, out3]

def check_g64_track18():
    with open("temp/Cauldron.g64", "rb") as f:
        f.read(8 + 1 + 1 + 2)
        track_offsets = []
        for i in range(84):
            track_offsets.append(int.from_bytes(f.read(4), 'little'))
            
        offset = track_offsets[34] # HT 34 = Track 18
        if offset == 0:
            print("Track 18 empty")
            return
            
        f.seek(offset)
        track_len = int.from_bytes(f.read(2), 'little')
        track_data = f.read(track_len)
        
        headers = []
        i = 0
        while i < len(track_data) - 10:
            if track_data[i] == 0xFF and track_data[i+1] == 0xFF:
                j = i
                while j < len(track_data) and track_data[j] == 0xFF:
                    j += 1
                if j + 5 <= len(track_data):
                    decoded = decode_5_bytes(track_data[j:j+5])
                    if decoded and decoded[0] == 0x08:
                        sector = decoded[2]
                        headers.append(sector)
                i = j
            else:
                i += 1
        print(f"Physical Sectors: {headers}")

check_g64_track18()
