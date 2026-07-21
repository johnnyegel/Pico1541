import sys

opcodes = {
    0x00: ('BRK', 1), 0x01: ('ORA', 2), 0x05: ('ORA', 2), 0x06: ('ASL', 2), 0x08: ('PHP', 1), 0x09: ('ORA', 2), 0x0a: ('ASL', 1), 0x0d: ('ORA', 3), 0x0e: ('ASL', 3),
    0x10: ('BPL', 2), 0x11: ('ORA', 2), 0x15: ('ORA', 2), 0x16: ('ASL', 2), 0x18: ('CLC', 1), 0x19: ('ORA', 3), 0x1d: ('ORA', 3), 0x1e: ('ASL', 3),
    0x20: ('JSR', 3), 0x21: ('AND', 2), 0x24: ('BIT', 2), 0x25: ('AND', 2), 0x26: ('ROL', 2), 0x28: ('PLP', 1), 0x29: ('AND', 2), 0x2a: ('ROL', 1), 0x2c: ('BIT', 3), 0x2d: ('AND', 3), 0x2e: ('ROL', 3),
    0x30: ('BMI', 2), 0x31: ('AND', 2), 0x35: ('AND', 2), 0x36: ('ROL', 2), 0x38: ('SEC', 1), 0x39: ('AND', 3), 0x3d: ('AND', 3), 0x3e: ('ROL', 3),
    0x40: ('RTI', 1), 0x41: ('EOR', 2), 0x45: ('EOR', 2), 0x46: ('LSR', 2), 0x48: ('PHA', 1), 0x49: ('EOR', 2), 0x4a: ('LSR', 1), 0x4c: ('JMP', 3), 0x4d: ('EOR', 3), 0x4e: ('LSR', 3),
    0x50: ('BVC', 2), 0x51: ('EOR', 2), 0x55: ('EOR', 2), 0x56: ('LSR', 2), 0x58: ('CLI', 1), 0x59: ('EOR', 3), 0x5d: ('EOR', 3), 0x5e: ('LSR', 3),
    0x60: ('RTS', 1), 0x61: ('ADC', 2), 0x65: ('ADC', 2), 0x66: ('ROR', 2), 0x68: ('PLA', 1), 0x69: ('ADC', 2), 0x6a: ('ROR', 1), 0x6c: ('JMP', 3), 0x6d: ('ADC', 3), 0x6e: ('ROR', 3),
    0x70: ('BVS', 2), 0x71: ('ADC', 2), 0x75: ('ADC', 2), 0x76: ('ROR', 2), 0x78: ('SEI', 1), 0x79: ('ADC', 3), 0x7d: ('ADC', 3), 0x7e: ('ROR', 3),
    0x81: ('STA', 2), 0x84: ('STY', 2), 0x85: ('STA', 2), 0x86: ('STX', 2), 0x88: ('DEY', 1), 0x8a: ('TXA', 1), 0x8c: ('STY', 3), 0x8d: ('STA', 3), 0x8e: ('STX', 3),
    0x90: ('BCC', 2), 0x91: ('STA', 2), 0x94: ('STY', 2), 0x95: ('STA', 2), 0x96: ('STX', 2), 0x98: ('TYA', 1), 0x99: ('STA', 3), 0x9a: ('TXS', 1), 0x9d: ('STA', 3),
    0xa0: ('LDY', 2), 0xa1: ('LDA', 2), 0xa2: ('LDX', 2), 0xa4: ('LDY', 2), 0xa5: ('LDA', 2), 0xa6: ('LDX', 2), 0xa8: ('TAY', 1), 0xa9: ('LDA', 2), 0xaa: ('TAX', 1), 0xac: ('LDY', 3), 0xad: ('LDA', 3), 0xae: ('LDX', 3),
    0xb0: ('BCS', 2), 0xb1: ('LDA', 2), 0xb4: ('LDY', 2), 0xb5: ('LDA', 2), 0xb6: ('LDX', 2), 0xb8: ('CLV', 1), 0xb9: ('LDA', 3), 0xba: ('TSX', 1), 0xbc: ('LDY', 3), 0xbd: ('LDA', 3), 0xbe: ('LDX', 3),
    0xc0: ('CPY', 2), 0xc1: ('CMP', 2), 0xc4: ('CPY', 2), 0xc5: ('CMP', 2), 0xc6: ('DEC', 2), 0xc8: ('INY', 1), 0xc9: ('CMP', 2), 0xca: ('DEX', 1), 0xcc: ('CPY', 3), 0xcd: ('CMP', 3), 0xce: ('DEC', 3),
    0xd0: ('BNE', 2), 0xd1: ('CMP', 2), 0xd5: ('CMP', 2), 0xd6: ('DEC', 2), 0xd8: ('CLD', 1), 0xd9: ('CMP', 3), 0xdd: ('CMP', 3), 0xde: ('DEC', 3),
    0xe0: ('CPX', 2), 0xe1: ('SBC', 2), 0xe4: ('CPX', 2), 0xe5: ('SBC', 2), 0xe6: ('INC', 2), 0xe8: ('INX', 1), 0xe9: ('SBC', 2), 0xea: ('NOP', 1), 0xec: ('CPX', 3), 0xed: ('SBC', 3), 0xee: ('INC', 3),
    0xf0: ('BEQ', 2), 0xf1: ('SBC', 2), 0xf5: ('SBC', 2), 0xf6: ('INC', 2), 0xf8: ('SED', 1), 0xf9: ('SBC', 3), 0xfd: ('SBC', 3), 0xfe: ('INC', 3)
}

with open(sys.argv[1], 'rb') as f:
    rom = f.read()

pc = int(sys.argv[2], 16) - 0xc000
end = int(sys.argv[3], 16) - 0xc000

while pc < end:
    b = rom[pc]
    if b in opcodes:
        name, length = opcodes[b]
        args = rom[pc+1:pc+length]
        arg_str = ' '.join(f'{x:02X}' for x in args)
        print(f'{pc+0xc000:04X}: {b:02X} {arg_str:<5} {name}')
        pc += length
    else:
        print(f'{pc+0xc000:04X}: {b:02X}       ???')
        pc += 1
