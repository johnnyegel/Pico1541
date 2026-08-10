import sys

def hex_search():
    with open("Testimages/Cauldron-g64.g64", "rb") as f:
        data = f.read()
    
    target = bytes([0x4A, 0xBE, 0x02, 0x00, 0x80])
    idx = data.find(target)
    if idx != -1:
        print(f"Found at offset {idx}")
    else:
        print("Not found in original binary!")

hex_search()
