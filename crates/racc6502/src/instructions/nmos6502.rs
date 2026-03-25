use super::{Instruction, AddrMicroOp, ExecMicroOp};

// --- Addressing Mode Slices ---
pub static ADDR_NONE: &[AddrMicroOp] = &[];
pub static ADDR_IMM: &[AddrMicroOp] = &[]; // Operand fetched in operation
pub static ADDR_ZP: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo];
pub static ADDR_ZPX: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToZP];
pub static ADDR_ZPY: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddYToZP];
pub static ADDR_ABS: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHi];
pub static ADDR_ABSX: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHi, AddrMicroOp::AddXToAbs];
pub static ADDR_ABSY: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHi, AddrMicroOp::AddYToAbs];

// --- Operation Slices ---
pub static OP_LDA_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndLDA];
pub static OP_LDA_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndLDA];
pub static OP_LDX_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndLDX];
pub static OP_LDX_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndLDX];
pub static OP_LDY_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndLDY];
pub static OP_LDY_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndLDY];

pub static OP_STA_MEM: &[ExecMicroOp] = &[ExecMicroOp::WriteEffA];
pub static OP_STX_MEM: &[ExecMicroOp] = &[ExecMicroOp::WriteEffX];
pub static OP_STY_MEM: &[ExecMicroOp] = &[ExecMicroOp::WriteEffY];

pub static OP_ADC_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndADC];
pub static OP_ADC_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndADC];
pub static OP_SBC_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndSBC];
pub static OP_SBC_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndSBC];
pub static OP_AND_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndAND];
pub static OP_AND_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndAND];
pub static OP_ORA_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndORA];
pub static OP_ORA_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndORA];
pub static OP_EOR_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndEOR];
pub static OP_EOR_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndEOR];

pub static OP_CMP_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndCMP];
pub static OP_CMP_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndCMP];
pub static OP_CPX_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndCPX];
pub static OP_CPX_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndCPX];
pub static OP_CPY_IMM: &[ExecMicroOp] = &[ExecMicroOp::FetchImmAndCPY];
pub static OP_CPY_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndCPY];

pub static OP_BIT_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffAndBIT];

pub static OP_TAX: &[ExecMicroOp] = &[ExecMicroOp::TAX];
pub static OP_TXA: &[ExecMicroOp] = &[ExecMicroOp::TXA];
pub static OP_TAY: &[ExecMicroOp] = &[ExecMicroOp::TAY];
pub static OP_TYA: &[ExecMicroOp] = &[ExecMicroOp::TYA];
pub static OP_TSX: &[ExecMicroOp] = &[ExecMicroOp::TSX];
pub static OP_TXS: &[ExecMicroOp] = &[ExecMicroOp::TXS];

pub static OP_INX: &[ExecMicroOp] = &[ExecMicroOp::INX];
pub static OP_DEX: &[ExecMicroOp] = &[ExecMicroOp::DEX];
pub static OP_INY: &[ExecMicroOp] = &[ExecMicroOp::INY];
pub static OP_DEY: &[ExecMicroOp] = &[ExecMicroOp::DEY];

pub static OP_CLC: &[ExecMicroOp] = &[ExecMicroOp::CLC];
pub static OP_SEC: &[ExecMicroOp] = &[ExecMicroOp::SEC];
pub static OP_CLI: &[ExecMicroOp] = &[ExecMicroOp::CLI];
pub static OP_SEI: &[ExecMicroOp] = &[ExecMicroOp::SEI];
pub static OP_CLV: &[ExecMicroOp] = &[ExecMicroOp::CLV];
pub static OP_CLD: &[ExecMicroOp] = &[ExecMicroOp::CLD];
pub static OP_SED: &[ExecMicroOp] = &[ExecMicroOp::SED];

pub static OP_NOP: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC];

pub static OP_UNIMPL: &[ExecMicroOp] = &[ExecMicroOp::Unimplemented];

const fn init_table() -> [Instruction; 256] {
    let mut table = [Instruction {
        addr: ADDR_NONE,
        exec: OP_UNIMPL,
    }; 256];

    // --- LDA ---
    table[0xA9] = Instruction { addr: ADDR_IMM, exec: OP_LDA_IMM };
    table[0xA5] = Instruction { addr: ADDR_ZP,  exec: OP_LDA_MEM };
    table[0xB5] = Instruction { addr: ADDR_ZPX, exec: OP_LDA_MEM };
    table[0xAD] = Instruction { addr: ADDR_ABS, exec: OP_LDA_MEM };
    table[0xBD] = Instruction { addr: ADDR_ABSX, exec: OP_LDA_MEM };
    table[0xB9] = Instruction { addr: ADDR_ABSY, exec: OP_LDA_MEM };

    // --- LDX ---
    table[0xA2] = Instruction { addr: ADDR_IMM, exec: OP_LDX_IMM };
    table[0xA6] = Instruction { addr: ADDR_ZP,  exec: OP_LDX_MEM };
    table[0xB6] = Instruction { addr: ADDR_ZPY, exec: OP_LDX_MEM };
    table[0xAE] = Instruction { addr: ADDR_ABS, exec: OP_LDX_MEM };
    table[0xBE] = Instruction { addr: ADDR_ABSY, exec: OP_LDX_MEM };

    // --- LDY ---
    table[0xA0] = Instruction { addr: ADDR_IMM, exec: OP_LDY_IMM };
    table[0xA4] = Instruction { addr: ADDR_ZP,  exec: OP_LDY_MEM };
    table[0xB4] = Instruction { addr: ADDR_ZPX, exec: OP_LDY_MEM };
    table[0xAC] = Instruction { addr: ADDR_ABS, exec: OP_LDY_MEM };
    table[0xBC] = Instruction { addr: ADDR_ABSX, exec: OP_LDY_MEM };

    // --- STA ---
    table[0x85] = Instruction { addr: ADDR_ZP,  exec: OP_STA_MEM };
    table[0x95] = Instruction { addr: ADDR_ZPX, exec: OP_STA_MEM };
    table[0x8D] = Instruction { addr: ADDR_ABS, exec: OP_STA_MEM };
    table[0x9D] = Instruction { addr: ADDR_ABSX, exec: OP_STA_MEM };
    table[0x99] = Instruction { addr: ADDR_ABSY, exec: OP_STA_MEM };

    // --- STX ---
    table[0x86] = Instruction { addr: ADDR_ZP,  exec: OP_STX_MEM };
    table[0x96] = Instruction { addr: ADDR_ZPY, exec: OP_STX_MEM };
    table[0x8E] = Instruction { addr: ADDR_ABS, exec: OP_STX_MEM };

    // --- STY ---
    table[0x84] = Instruction { addr: ADDR_ZP,  exec: OP_STY_MEM };
    table[0x94] = Instruction { addr: ADDR_ZPX, exec: OP_STY_MEM };
    table[0x8C] = Instruction { addr: ADDR_ABS, exec: OP_STY_MEM };

    // --- ADC ---
    table[0x69] = Instruction { addr: ADDR_IMM, exec: OP_ADC_IMM };
    table[0x65] = Instruction { addr: ADDR_ZP,  exec: OP_ADC_MEM };
    table[0x75] = Instruction { addr: ADDR_ZPX, exec: OP_ADC_MEM };
    table[0x6D] = Instruction { addr: ADDR_ABS, exec: OP_ADC_MEM };

    // --- SBC ---
    table[0xE9] = Instruction { addr: ADDR_IMM, exec: OP_SBC_IMM };
    table[0xE5] = Instruction { addr: ADDR_ZP,  exec: OP_SBC_MEM };
    table[0xF5] = Instruction { addr: ADDR_ZPX, exec: OP_SBC_MEM };
    table[0xED] = Instruction { addr: ADDR_ABS, exec: OP_SBC_MEM };

    // --- Transf / Inc / Dec ---
    table[0xAA] = Instruction { addr: ADDR_NONE, exec: OP_TAX };
    table[0x8A] = Instruction { addr: ADDR_NONE, exec: OP_TXA };
    table[0xA8] = Instruction { addr: ADDR_NONE, exec: OP_TAY };
    table[0x98] = Instruction { addr: ADDR_NONE, exec: OP_TYA };
    table[0xBA] = Instruction { addr: ADDR_NONE, exec: OP_TSX };
    table[0x9A] = Instruction { addr: ADDR_NONE, exec: OP_TXS };
    table[0xE8] = Instruction { addr: ADDR_NONE, exec: OP_INX };
    table[0xCA] = Instruction { addr: ADDR_NONE, exec: OP_DEX };
    table[0xC8] = Instruction { addr: ADDR_NONE, exec: OP_INY };
    table[0x88] = Instruction { addr: ADDR_NONE, exec: OP_DEY };

    // --- Flags ---
    table[0x18] = Instruction { addr: ADDR_NONE, exec: OP_CLC };
    table[0x38] = Instruction { addr: ADDR_NONE, exec: OP_SEC };
    table[0x58] = Instruction { addr: ADDR_NONE, exec: OP_CLI };
    table[0x78] = Instruction { addr: ADDR_NONE, exec: OP_SEI };
    table[0xB8] = Instruction { addr: ADDR_NONE, exec: OP_CLV };
    table[0xD8] = Instruction { addr: ADDR_NONE, exec: OP_CLD };
    table[0xF8] = Instruction { addr: ADDR_NONE, exec: OP_SED };

    // --- NOP ---
    table[0xEA] = Instruction { addr: ADDR_NONE, exec: OP_NOP };

    table
}

pub static INSTRUCTION_TABLE: [Instruction; 256] = init_table();
