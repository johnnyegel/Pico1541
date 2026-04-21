//! # NMOS 6502 Instruction Set
//! 
//! This module defines the mapping of all 151 legal NMOS 6502 opcodes to their
//! cycle-accurate micro-operation sequences.

use super::{Instruction, AddrMicroOp, ExecMicroOp};

// --- Addressing Mode Slices ---
// These slices define the bus activity for each 6502 addressing mode.

/// Implied or Accumulator addressing (0 extra cycles).
pub static ADDR_NONE: &[AddrMicroOp] = &[];
/// Immediate addressing (operand fetched during execution).
pub static ADDR_IMM: &[AddrMicroOp] = &[]; 
/// Zero Page addressing (1 cycle: fetch ZP address).
pub static ADDR_ZP: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo];
/// Zero Page,X addressing (2 cycles: fetch address, add X).
pub static ADDR_ZPX: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToZP];
/// Zero Page,Y addressing (2 cycles: fetch address, add Y).
pub static ADDR_ZPY: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddYToZP];
/// Absolute addressing (2 cycles: fetch low, fetch high).
pub static ADDR_ABS: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHi];
/// Special variant for JMP Abs (2 cycles: fetch low, fetch high and update PC).
pub static ADDR_ABS_JMP: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHiAndJump];
/// Absolute,X addressing for read instructions (3+ cycles: fetch low, fetch high + add X, optional fixup).
pub static ADDR_ABSX_READ: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToAbs, AddrMicroOp::OptionalFixupX];
/// Absolute,X addressing for write/RMW instructions (4 cycles: mandatory dummy read/fixup).
pub static ADDR_ABSX_WRITE: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToAbs, AddrMicroOp::MandatoryFixupX];
/// Absolute,Y addressing for read instructions (3+ cycles).
pub static ADDR_ABSY_READ: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddYToAbs, AddrMicroOp::OptionalFixupY];
/// Absolute,Y addressing for write/RMW instructions (4 cycles).
pub static ADDR_ABSY_WRITE: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddYToAbs, AddrMicroOp::MandatoryFixupY];
/// Indirect,X addressing (5 cycles).
pub static ADDR_INDX: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToZP, AddrMicroOp::FetchIndXLo, AddrMicroOp::FetchIndXHi];
/// Indirect,Y addressing for read instructions (4+ cycles).
pub static ADDR_INDY_READ: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchIndYLo, AddrMicroOp::FetchIndYHi, AddrMicroOp::OptionalFixupIndY];
/// Indirect,Y addressing for write instructions (5 cycles).
pub static ADDR_INDY_WRITE: &[AddrMicroOp] = &[AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchIndYLo, AddrMicroOp::FetchIndYHi, AddrMicroOp::MandatoryFixupIndY];

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

pub static OP_ASL_ACC: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::AccASL];
pub static OP_ASL_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::ASLWrite];
pub static OP_LSR_ACC: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::AccLSR];
pub static OP_LSR_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::LSRWrite];
pub static OP_ROL_ACC: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::AccROL];
pub static OP_ROL_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::ROLWrite];
pub static OP_ROR_ACC: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::AccROR];
pub static OP_ROR_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::RORWrite];

pub static OP_INC_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::INCWrite];
pub static OP_DEC_MEM: &[ExecMicroOp] = &[ExecMicroOp::ReadEffToData, ExecMicroOp::WriteDataBack, ExecMicroOp::DECWrite];

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

pub static OP_JMP_ABS: &[ExecMicroOp] = &[];
pub static OP_JMP_IND: &[ExecMicroOp] = &[ExecMicroOp::JumpIndCycle4, ExecMicroOp::JumpIndCycle5];

pub static OP_BRANCH: &[ExecMicroOp] = &[ExecMicroOp::BranchCycle2, ExecMicroOp::BranchCycle3, ExecMicroOp::BranchCycle4];

pub static OP_JSR: &[ExecMicroOp] = &[
    ExecMicroOp::JsrCycle2, ExecMicroOp::JsrCycle3, ExecMicroOp::JsrCycle4, 
    ExecMicroOp::JsrCycle5, ExecMicroOp::JsrCycle6
];

pub static OP_RTS: &[ExecMicroOp] = &[
    ExecMicroOp::RtsCycle2, ExecMicroOp::RtsCycle3, ExecMicroOp::RtsCycle4, 
    ExecMicroOp::RtsCycle5, ExecMicroOp::RtsCycle6
];

pub static OP_RTI: &[ExecMicroOp] = &[
    ExecMicroOp::RtiCycle2, ExecMicroOp::RtiCycle3, ExecMicroOp::RtiCycle4, 
    ExecMicroOp::RtiCycle5, ExecMicroOp::RtiCycle6
];

pub static OP_BRK: &[ExecMicroOp] = &[
    ExecMicroOp::BrkCycle2, ExecMicroOp::BrkCycle3, ExecMicroOp::BrkCycle4,
    ExecMicroOp::BrkCycle5, ExecMicroOp::BrkCycle6, ExecMicroOp::BrkCycle7
];

pub static OP_PHA: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::PushA];
pub static OP_PHP: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::PushP];
pub static OP_PLA: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::DummyReadStack, ExecMicroOp::PullA];
pub static OP_PLP: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC, ExecMicroOp::DummyReadStack, ExecMicroOp::PullP];

pub static OP_NOP: &[ExecMicroOp] = &[ExecMicroOp::DummyReadPC];

pub static OP_NONE: &[ExecMicroOp] = &[];
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
    table[0xBD] = Instruction { addr: ADDR_ABSX_READ, exec: OP_LDA_MEM };
    table[0xB9] = Instruction { addr: ADDR_ABSY_READ, exec: OP_LDA_MEM };
    table[0xA1] = Instruction { addr: ADDR_INDX, exec: OP_LDA_MEM };
    table[0xB1] = Instruction { addr: ADDR_INDY_READ, exec: OP_LDA_MEM };

    // --- LDX ---
    table[0xA2] = Instruction { addr: ADDR_IMM, exec: OP_LDX_IMM };
    table[0xA6] = Instruction { addr: ADDR_ZP,  exec: OP_LDX_MEM };
    table[0xB6] = Instruction { addr: ADDR_ZPY, exec: OP_LDX_MEM };
    table[0xAE] = Instruction { addr: ADDR_ABS, exec: OP_LDX_MEM };
    table[0xBE] = Instruction { addr: ADDR_ABSY_READ, exec: OP_LDX_MEM };

    // --- LDY ---
    table[0xA0] = Instruction { addr: ADDR_IMM, exec: OP_LDY_IMM };
    table[0xA4] = Instruction { addr: ADDR_ZP,  exec: OP_LDY_MEM };
    table[0xB4] = Instruction { addr: ADDR_ZPX, exec: OP_LDY_MEM };
    table[0xAC] = Instruction { addr: ADDR_ABS, exec: OP_LDY_MEM };
    table[0xBC] = Instruction { addr: ADDR_ABSX_READ, exec: OP_LDY_MEM };

    // --- STA ---
    table[0x85] = Instruction { addr: ADDR_ZP,  exec: OP_STA_MEM };
    table[0x95] = Instruction { addr: ADDR_ZPX, exec: OP_STA_MEM };
    table[0x8D] = Instruction { addr: ADDR_ABS, exec: OP_STA_MEM };
    table[0x9D] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_STA_MEM };
    table[0x99] = Instruction { addr: ADDR_ABSY_WRITE, exec: OP_STA_MEM };
    table[0x81] = Instruction { addr: ADDR_INDX, exec: OP_STA_MEM };
    table[0x91] = Instruction { addr: ADDR_INDY_WRITE, exec: OP_STA_MEM };

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
    table[0x7D] = Instruction { addr: ADDR_ABSX_READ, exec: OP_ADC_MEM };
    table[0x79] = Instruction { addr: ADDR_ABSY_READ, exec: OP_ADC_MEM };
    table[0x61] = Instruction { addr: ADDR_INDX, exec: OP_ADC_MEM };
    table[0x71] = Instruction { addr: ADDR_INDY_READ, exec: OP_ADC_MEM };

    // --- SBC ---
    table[0xE9] = Instruction { addr: ADDR_IMM, exec: OP_SBC_IMM };
    table[0xE5] = Instruction { addr: ADDR_ZP,  exec: OP_SBC_MEM };
    table[0xF5] = Instruction { addr: ADDR_ZPX, exec: OP_SBC_MEM };
    table[0xED] = Instruction { addr: ADDR_ABS, exec: OP_SBC_MEM };
    table[0xFD] = Instruction { addr: ADDR_ABSX_READ, exec: OP_SBC_MEM };
    table[0xF9] = Instruction { addr: ADDR_ABSY_READ, exec: OP_SBC_MEM };
    table[0xE1] = Instruction { addr: ADDR_INDX, exec: OP_SBC_MEM };
    table[0xF1] = Instruction { addr: ADDR_INDY_READ, exec: OP_SBC_MEM };

    // --- AND ---
    table[0x29] = Instruction { addr: ADDR_IMM, exec: OP_AND_IMM };
    table[0x25] = Instruction { addr: ADDR_ZP,  exec: OP_AND_MEM };
    table[0x35] = Instruction { addr: ADDR_ZPX, exec: OP_AND_MEM };
    table[0x2D] = Instruction { addr: ADDR_ABS, exec: OP_AND_MEM };
    table[0x3D] = Instruction { addr: ADDR_ABSX_READ, exec: OP_AND_MEM };
    table[0x39] = Instruction { addr: ADDR_ABSY_READ, exec: OP_AND_MEM };
    table[0x21] = Instruction { addr: ADDR_INDX, exec: OP_AND_MEM };
    table[0x31] = Instruction { addr: ADDR_INDY_READ, exec: OP_AND_MEM };

    // --- ORA ---
    table[0x09] = Instruction { addr: ADDR_IMM, exec: OP_ORA_IMM };
    table[0x05] = Instruction { addr: ADDR_ZP,  exec: OP_ORA_MEM };
    table[0x15] = Instruction { addr: ADDR_ZPX, exec: OP_ORA_MEM };
    table[0x0D] = Instruction { addr: ADDR_ABS, exec: OP_ORA_MEM };
    table[0x1D] = Instruction { addr: ADDR_ABSX_READ, exec: OP_ORA_MEM };
    table[0x19] = Instruction { addr: ADDR_ABSY_READ, exec: OP_ORA_MEM };
    table[0x01] = Instruction { addr: ADDR_INDX, exec: OP_ORA_MEM };
    table[0x11] = Instruction { addr: ADDR_INDY_READ, exec: OP_ORA_MEM };

    // --- EOR ---
    table[0x49] = Instruction { addr: ADDR_IMM, exec: OP_EOR_IMM };
    table[0x45] = Instruction { addr: ADDR_ZP,  exec: OP_EOR_MEM };
    table[0x55] = Instruction { addr: ADDR_ZPX, exec: OP_EOR_MEM };
    table[0x4D] = Instruction { addr: ADDR_ABS, exec: OP_EOR_MEM };
    table[0x5D] = Instruction { addr: ADDR_ABSX_READ, exec: OP_EOR_MEM };
    table[0x59] = Instruction { addr: ADDR_ABSY_READ, exec: OP_EOR_MEM };
    table[0x41] = Instruction { addr: ADDR_INDX, exec: OP_EOR_MEM };
    table[0x51] = Instruction { addr: ADDR_INDY_READ, exec: OP_EOR_MEM };

    // --- BIT ---
    table[0x24] = Instruction { addr: ADDR_ZP,  exec: OP_BIT_MEM };
    table[0x2C] = Instruction { addr: ADDR_ABS, exec: OP_BIT_MEM };

    // --- CMP ---
    table[0xC9] = Instruction { addr: ADDR_IMM, exec: OP_CMP_IMM };
    table[0xC5] = Instruction { addr: ADDR_ZP,  exec: OP_CMP_MEM };
    table[0xD5] = Instruction { addr: ADDR_ZPX, exec: OP_CMP_MEM };
    table[0xCD] = Instruction { addr: ADDR_ABS, exec: OP_CMP_MEM };
    table[0xDD] = Instruction { addr: ADDR_ABSX_READ, exec: OP_CMP_MEM };
    table[0xD9] = Instruction { addr: ADDR_ABSY_READ, exec: OP_CMP_MEM };
    table[0xC1] = Instruction { addr: ADDR_INDX, exec: OP_CMP_MEM };
    table[0xD1] = Instruction { addr: ADDR_INDY_READ, exec: OP_CMP_MEM };

    // --- CPX ---
    table[0xE0] = Instruction { addr: ADDR_IMM, exec: OP_CPX_IMM };
    table[0xE4] = Instruction { addr: ADDR_ZP,  exec: OP_CPX_MEM };
    table[0xEC] = Instruction { addr: ADDR_ABS, exec: OP_CPX_MEM };

    // --- CPY ---
    table[0xC0] = Instruction { addr: ADDR_IMM, exec: OP_CPY_IMM };
    table[0xC4] = Instruction { addr: ADDR_ZP,  exec: OP_CPY_MEM };
    table[0xCC] = Instruction { addr: ADDR_ABS, exec: OP_CPY_MEM };

    // --- ASL ---
    table[0x0A] = Instruction { addr: ADDR_NONE, exec: OP_ASL_ACC };
    table[0x06] = Instruction { addr: ADDR_ZP,   exec: OP_ASL_MEM };
    table[0x16] = Instruction { addr: ADDR_ZPX,  exec: OP_ASL_MEM };
    table[0x0E] = Instruction { addr: ADDR_ABS,  exec: OP_ASL_MEM };
    table[0x1E] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_ASL_MEM };

    // --- LSR ---
    table[0x4A] = Instruction { addr: ADDR_NONE, exec: OP_LSR_ACC };
    table[0x46] = Instruction { addr: ADDR_ZP,   exec: OP_LSR_MEM };
    table[0x56] = Instruction { addr: ADDR_ZPX,  exec: OP_LSR_MEM };
    table[0x4E] = Instruction { addr: ADDR_ABS,  exec: OP_LSR_MEM };
    table[0x5E] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_LSR_MEM };

    // --- ROL ---
    table[0x2A] = Instruction { addr: ADDR_NONE, exec: OP_ROL_ACC };
    table[0x26] = Instruction { addr: ADDR_ZP,   exec: OP_ROL_MEM };
    table[0x36] = Instruction { addr: ADDR_ZPX,  exec: OP_ROL_MEM };
    table[0x2E] = Instruction { addr: ADDR_ABS,  exec: OP_ROL_MEM };
    table[0x3E] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_ROL_MEM };

    // --- ROR ---
    table[0x6A] = Instruction { addr: ADDR_NONE, exec: OP_ROR_ACC };
    table[0x66] = Instruction { addr: ADDR_ZP,   exec: OP_ROR_MEM };
    table[0x76] = Instruction { addr: ADDR_ZPX,  exec: OP_ROR_MEM };
    table[0x6E] = Instruction { addr: ADDR_ABS,  exec: OP_ROR_MEM };
    table[0x7E] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_ROR_MEM };

    // --- INC ---
    table[0xE6] = Instruction { addr: ADDR_ZP,   exec: OP_INC_MEM };
    table[0xF6] = Instruction { addr: ADDR_ZPX,  exec: OP_INC_MEM };
    table[0xEE] = Instruction { addr: ADDR_ABS,  exec: OP_INC_MEM };
    table[0xFE] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_INC_MEM };

    // --- DEC ---
    table[0xC6] = Instruction { addr: ADDR_ZP,   exec: OP_DEC_MEM };
    table[0xD6] = Instruction { addr: ADDR_ZPX,  exec: OP_DEC_MEM };
    table[0xCE] = Instruction { addr: ADDR_ABS,  exec: OP_DEC_MEM };
    table[0xDE] = Instruction { addr: ADDR_ABSX_WRITE, exec: OP_DEC_MEM };

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

    // --- Jump / Branch / Subroutine ---
    table[0x4C] = Instruction { addr: ADDR_ABS_JMP, exec: OP_NONE };
    table[0x6C] = Instruction { addr: ADDR_ABS,     exec: OP_JMP_IND };
    table[0x20] = Instruction { addr: ADDR_NONE,    exec: OP_JSR };
    table[0x60] = Instruction { addr: ADDR_NONE,    exec: OP_RTS };
    table[0x40] = Instruction { addr: ADDR_NONE,    exec: OP_RTI };
    table[0x00] = Instruction { addr: ADDR_NONE,    exec: OP_BRK };

    table[0x10] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0x30] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0x50] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0x70] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0x90] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0xB0] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0xD0] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };
    table[0xF0] = Instruction { addr: ADDR_NONE, exec: OP_BRANCH };

    // --- Stack ---
    table[0x08] = Instruction { addr: ADDR_NONE, exec: OP_PHP };
    table[0x28] = Instruction { addr: ADDR_NONE, exec: OP_PLP };
    table[0x48] = Instruction { addr: ADDR_NONE, exec: OP_PHA };
    table[0x68] = Instruction { addr: ADDR_NONE, exec: OP_PLA };

    // --- NOP ---
    table[0xEA] = Instruction { addr: ADDR_NONE, exec: OP_NOP };

    table
}

pub static INSTRUCTION_TABLE: [Instruction; 256] = init_table();
