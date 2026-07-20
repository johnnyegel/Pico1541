import sys

payload = bytes.fromhex("78 20 8F 01 A9 7A 8D 02 18 20 E9 F5 20 62 01 4C 00 03 A0 00 A9 00 8D 00 18 AD 00 18 D0 FB 08 AD 00 18 0A 28 4D 00 18 0A 0A 0A EA EA EA 4D 00 18 0A EA EA EA 4D 00 18 99 00 03 C8 EA EA D0 DA A9 08 8D 00 18 60")

def disasm(data, start_addr):
    # A simple 6502 disassembler is not trivial to write inline.
    # Instead we will use a python module if available, or write a simple script to just decode these bytes.
    pass
