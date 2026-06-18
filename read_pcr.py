def check_pcr():
    pcr = 0xec
    print(f"PCR2 = {pcr:02X}")
    print(f"CB2: {(pcr >> 5) & 7:03b}")
    print(f"CB1: {(pcr >> 4) & 1:b}")
    print(f"CA2: {(pcr >> 1) & 7:03b}")
    print(f"CA1: {pcr & 1:b}")

check_pcr()
