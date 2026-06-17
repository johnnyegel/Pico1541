rom = open("flash-fs/roms/1541-II.251968-03.bin", "rb").read()
# Rom is mapped at $C000-$FFFF. So offset is addr - 0xC000
for addr in range(0xEDDD, 0xEE0D):
    if addr - 0xC000 < len(rom):
        b = rom[addr - 0xC000]
        print(f"${addr:04X}: {b:02X}")
