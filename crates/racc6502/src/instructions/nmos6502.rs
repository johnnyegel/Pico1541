//! # NMOS 6502 Instruction Set
//!
//! This module defines the mapping of all 151 legal NMOS 6502 opcodes to their
//! cycle-accurate micro-operation sequences.

use super::{AddrMicroOp, ExecMicroOp, Instruction};

// --- Addressing Mode Slices ---
// These slices define the bus activity for each 6502 addressing mode.

/// Implied or Accumulator addressing (0 extra cycles).
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_NONE: &[AddrMicroOp] = &[];
/// Immediate addressing (operand fetched during execution).
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_IMM: &[AddrMicroOp] = &[];

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ZP_ARR: [AddrMicroOp; 1] = [AddrMicroOp::FetchAddrLo];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ZP: &[AddrMicroOp] = &ADDR_ZP_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ZPX_ARR: [AddrMicroOp; 2] = [AddrMicroOp::FetchAddrLo, AddrMicroOp::AddXToZP];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ZPX: &[AddrMicroOp] = &ADDR_ZPX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ZPY_ARR: [AddrMicroOp; 2] = [AddrMicroOp::FetchAddrLo, AddrMicroOp::AddYToZP];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ZPY: &[AddrMicroOp] = &ADDR_ZPY_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABS_ARR: [AddrMicroOp; 2] = [AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHi];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABS: &[AddrMicroOp] = &ADDR_ABS_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABS_JMP_ARR: [AddrMicroOp; 2] = [AddrMicroOp::FetchAddrLo, AddrMicroOp::FetchAddrHiAndJump];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABS_JMP: &[AddrMicroOp] = &ADDR_ABS_JMP_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABSX_READ_ARR: [AddrMicroOp; 3] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::AddXToAbs,
    AddrMicroOp::OptionalFixupX,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABSX_READ: &[AddrMicroOp] = &ADDR_ABSX_READ_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABSX_WRITE_ARR: [AddrMicroOp; 3] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::AddXToAbs,
    AddrMicroOp::MandatoryFixupX,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABSX_WRITE: &[AddrMicroOp] = &ADDR_ABSX_WRITE_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABSY_READ_ARR: [AddrMicroOp; 3] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::AddYToAbs,
    AddrMicroOp::OptionalFixupY,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABSY_READ: &[AddrMicroOp] = &ADDR_ABSY_READ_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_ABSY_WRITE_ARR: [AddrMicroOp; 3] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::AddYToAbs,
    AddrMicroOp::MandatoryFixupY,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_ABSY_WRITE: &[AddrMicroOp] = &ADDR_ABSY_WRITE_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_INDX_ARR: [AddrMicroOp; 4] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::AddXToZP,
    AddrMicroOp::FetchIndXLo,
    AddrMicroOp::FetchIndXHi,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_INDX: &[AddrMicroOp] = &ADDR_INDX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_INDY_READ_ARR: [AddrMicroOp; 4] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::FetchIndYLo,
    AddrMicroOp::FetchIndYHi,
    AddrMicroOp::OptionalFixupIndY,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_INDY_READ: &[AddrMicroOp] = &ADDR_INDY_READ_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static ADDR_INDY_WRITE_ARR: [AddrMicroOp; 4] = [
    AddrMicroOp::FetchAddrLo,
    AddrMicroOp::FetchIndYLo,
    AddrMicroOp::FetchIndYHi,
    AddrMicroOp::MandatoryFixupIndY,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static ADDR_INDY_WRITE: &[AddrMicroOp] = &ADDR_INDY_WRITE_ARR;

// --- Operation Slices ---
#[link_section = ".data.pico1541_emulator_core"]
static OP_LDA_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndLDA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDA_IMM: &[ExecMicroOp] = &OP_LDA_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LDA_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndLDA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDA_MEM: &[ExecMicroOp] = &OP_LDA_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LDX_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndLDX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDX_IMM: &[ExecMicroOp] = &OP_LDX_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LDX_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndLDX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDX_MEM: &[ExecMicroOp] = &OP_LDX_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LDY_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndLDY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDY_IMM: &[ExecMicroOp] = &OP_LDY_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LDY_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndLDY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LDY_MEM: &[ExecMicroOp] = &OP_LDY_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_STA_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::WriteEffA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_STA_MEM: &[ExecMicroOp] = &OP_STA_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_STX_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::WriteEffX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_STX_MEM: &[ExecMicroOp] = &OP_STX_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_STY_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::WriteEffY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_STY_MEM: &[ExecMicroOp] = &OP_STY_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ADC_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndADC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ADC_IMM: &[ExecMicroOp] = &OP_ADC_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ADC_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndADC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ADC_MEM: &[ExecMicroOp] = &OP_ADC_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_SBC_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndSBC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_SBC_IMM: &[ExecMicroOp] = &OP_SBC_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_SBC_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndSBC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_SBC_MEM: &[ExecMicroOp] = &OP_SBC_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_AND_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndAND];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_AND_IMM: &[ExecMicroOp] = &OP_AND_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_AND_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndAND];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_AND_MEM: &[ExecMicroOp] = &OP_AND_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ORA_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndORA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ORA_IMM: &[ExecMicroOp] = &OP_ORA_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ORA_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndORA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ORA_MEM: &[ExecMicroOp] = &OP_ORA_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_EOR_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndEOR];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_EOR_IMM: &[ExecMicroOp] = &OP_EOR_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_EOR_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndEOR];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_EOR_MEM: &[ExecMicroOp] = &OP_EOR_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CMP_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndCMP];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CMP_IMM: &[ExecMicroOp] = &OP_CMP_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CMP_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndCMP];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CMP_MEM: &[ExecMicroOp] = &OP_CMP_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CPX_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndCPX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CPX_IMM: &[ExecMicroOp] = &OP_CPX_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CPX_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndCPX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CPX_MEM: &[ExecMicroOp] = &OP_CPX_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CPY_IMM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::FetchImmAndCPY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CPY_IMM: &[ExecMicroOp] = &OP_CPY_IMM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CPY_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndCPY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CPY_MEM: &[ExecMicroOp] = &OP_CPY_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_BIT_MEM_ARR: [ExecMicroOp; 1] = [ExecMicroOp::ReadEffAndBIT];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_BIT_MEM: &[ExecMicroOp] = &OP_BIT_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ASL_ACC_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::AccASL];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ASL_ACC: &[ExecMicroOp] = &OP_ASL_ACC_ARR;
#[link_section = ".data.pico1541_emulator_core"]
static OP_ASL_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::ASLWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ASL_MEM: &[ExecMicroOp] = &OP_ASL_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LSR_ACC_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::AccLSR];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LSR_ACC: &[ExecMicroOp] = &OP_LSR_ACC_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_LSR_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::LSRWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_LSR_MEM: &[ExecMicroOp] = &OP_LSR_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ROL_ACC_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::AccROL];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ROL_ACC: &[ExecMicroOp] = &OP_ROL_ACC_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ROL_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::ROLWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ROL_MEM: &[ExecMicroOp] = &OP_ROL_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ROR_ACC_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::AccROR];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ROR_ACC: &[ExecMicroOp] = &OP_ROR_ACC_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_ROR_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::RORWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_ROR_MEM: &[ExecMicroOp] = &OP_ROR_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_INC_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::INCWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_INC_MEM: &[ExecMicroOp] = &OP_INC_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_DEC_MEM_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::ReadEffToData,
    ExecMicroOp::WriteDataBack,
    ExecMicroOp::DECWrite,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_DEC_MEM: &[ExecMicroOp] = &OP_DEC_MEM_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TAX_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TAX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TAX: &[ExecMicroOp] = &OP_TAX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TXA_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TXA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TXA: &[ExecMicroOp] = &OP_TXA_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TAY_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TAY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TAY: &[ExecMicroOp] = &OP_TAY_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TYA_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TYA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TYA: &[ExecMicroOp] = &OP_TYA_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TSX_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TSX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TSX: &[ExecMicroOp] = &OP_TSX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_TXS_ARR: [ExecMicroOp; 1] = [ExecMicroOp::TXS];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_TXS: &[ExecMicroOp] = &OP_TXS_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_INX_ARR: [ExecMicroOp; 1] = [ExecMicroOp::INX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_INX: &[ExecMicroOp] = &OP_INX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_DEX_ARR: [ExecMicroOp; 1] = [ExecMicroOp::DEX];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_DEX: &[ExecMicroOp] = &OP_DEX_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_INY_ARR: [ExecMicroOp; 1] = [ExecMicroOp::INY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_INY: &[ExecMicroOp] = &OP_INY_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_DEY_ARR: [ExecMicroOp; 1] = [ExecMicroOp::DEY];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_DEY: &[ExecMicroOp] = &OP_DEY_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CLC_ARR: [ExecMicroOp; 1] = [ExecMicroOp::CLC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CLC: &[ExecMicroOp] = &OP_CLC_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_SEC_ARR: [ExecMicroOp; 1] = [ExecMicroOp::SEC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_SEC: &[ExecMicroOp] = &OP_SEC_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CLI_ARR: [ExecMicroOp; 1] = [ExecMicroOp::CLI];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CLI: &[ExecMicroOp] = &OP_CLI_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_SEI_ARR: [ExecMicroOp; 1] = [ExecMicroOp::SEI];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_SEI: &[ExecMicroOp] = &OP_SEI_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CLV_ARR: [ExecMicroOp; 1] = [ExecMicroOp::CLV];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CLV: &[ExecMicroOp] = &OP_CLV_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_CLD_ARR: [ExecMicroOp; 1] = [ExecMicroOp::CLD];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_CLD: &[ExecMicroOp] = &OP_CLD_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_SED_ARR: [ExecMicroOp; 1] = [ExecMicroOp::SED];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_SED: &[ExecMicroOp] = &OP_SED_ARR;

#[link_section = ".data.pico1541_emulator_core"]
pub static OP_JMP_ABS: &[ExecMicroOp] = &[];

#[link_section = ".data.pico1541_emulator_core"]
static OP_JMP_IND_ARR: [ExecMicroOp; 2] = [ExecMicroOp::JumpIndCycle4, ExecMicroOp::JumpIndCycle5];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_JMP_IND: &[ExecMicroOp] = &OP_JMP_IND_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_BRANCH_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::BranchCycle2,
    ExecMicroOp::BranchCycle3,
    ExecMicroOp::BranchCycle4,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_BRANCH: &[ExecMicroOp] = &OP_BRANCH_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_JSR_ARR: [ExecMicroOp; 5] = [
    ExecMicroOp::JsrCycle2,
    ExecMicroOp::JsrCycle3,
    ExecMicroOp::JsrCycle4,
    ExecMicroOp::JsrCycle5,
    ExecMicroOp::JsrCycle6,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_JSR: &[ExecMicroOp] = &OP_JSR_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_RTS_ARR: [ExecMicroOp; 5] = [
    ExecMicroOp::RtsCycle2,
    ExecMicroOp::RtsCycle3,
    ExecMicroOp::RtsCycle4,
    ExecMicroOp::RtsCycle5,
    ExecMicroOp::RtsCycle6,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_RTS: &[ExecMicroOp] = &OP_RTS_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_RTI_ARR: [ExecMicroOp; 5] = [
    ExecMicroOp::RtiCycle2,
    ExecMicroOp::RtiCycle3,
    ExecMicroOp::RtiCycle4,
    ExecMicroOp::RtiCycle5,
    ExecMicroOp::RtiCycle6,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_RTI: &[ExecMicroOp] = &OP_RTI_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_BRK_ARR: [ExecMicroOp; 6] = [
    ExecMicroOp::BrkCycle2,
    ExecMicroOp::BrkCycle3,
    ExecMicroOp::BrkCycle4,
    ExecMicroOp::BrkCycle5,
    ExecMicroOp::BrkCycle6,
    ExecMicroOp::BrkCycle7,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_BRK: &[ExecMicroOp] = &OP_BRK_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_PHA_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::PushA];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_PHA: &[ExecMicroOp] = &OP_PHA_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_PHP_ARR: [ExecMicroOp; 2] = [ExecMicroOp::DummyReadPC, ExecMicroOp::PushP];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_PHP: &[ExecMicroOp] = &OP_PHP_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_PLA_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::DummyReadPC,
    ExecMicroOp::DummyReadStack,
    ExecMicroOp::PullA,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_PLA: &[ExecMicroOp] = &OP_PLA_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_PLP_ARR: [ExecMicroOp; 3] = [
    ExecMicroOp::DummyReadPC,
    ExecMicroOp::DummyReadStack,
    ExecMicroOp::PullP,
];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_PLP: &[ExecMicroOp] = &OP_PLP_ARR;

#[link_section = ".data.pico1541_emulator_core"]
static OP_NOP_ARR: [ExecMicroOp; 1] = [ExecMicroOp::DummyReadPC];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_NOP: &[ExecMicroOp] = &OP_NOP_ARR;

#[link_section = ".data.pico1541_emulator_core"]
pub static OP_NONE: &[ExecMicroOp] = &[];

#[link_section = ".data.pico1541_emulator_core"]
static OP_UNIMPL_ARR: [ExecMicroOp; 1] = [ExecMicroOp::Unimplemented];
#[link_section = ".data.pico1541_emulator_core"]
pub static OP_UNIMPL: &[ExecMicroOp] = &OP_UNIMPL_ARR;

const fn init_table() -> [Instruction; 256] {
    let mut table = [Instruction {
        addr: ADDR_NONE,
        exec: OP_UNIMPL,
    }; 256];

    // --- LDA ---
    table[0xA9] = Instruction {
        addr: ADDR_IMM,
        exec: OP_LDA_IMM,
    };
    table[0xA5] = Instruction {
        addr: ADDR_ZP,
        exec: OP_LDA_MEM,
    };
    table[0xB5] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_LDA_MEM,
    };
    table[0xAD] = Instruction {
        addr: ADDR_ABS,
        exec: OP_LDA_MEM,
    };
    table[0xBD] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_LDA_MEM,
    };
    table[0xB9] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_LDA_MEM,
    };
    table[0xA1] = Instruction {
        addr: ADDR_INDX,
        exec: OP_LDA_MEM,
    };
    table[0xB1] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_LDA_MEM,
    };

    // --- LDX ---
    table[0xA2] = Instruction {
        addr: ADDR_IMM,
        exec: OP_LDX_IMM,
    };
    table[0xA6] = Instruction {
        addr: ADDR_ZP,
        exec: OP_LDX_MEM,
    };
    table[0xB6] = Instruction {
        addr: ADDR_ZPY,
        exec: OP_LDX_MEM,
    };
    table[0xAE] = Instruction {
        addr: ADDR_ABS,
        exec: OP_LDX_MEM,
    };
    table[0xBE] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_LDX_MEM,
    };

    // --- LDY ---
    table[0xA0] = Instruction {
        addr: ADDR_IMM,
        exec: OP_LDY_IMM,
    };
    table[0xA4] = Instruction {
        addr: ADDR_ZP,
        exec: OP_LDY_MEM,
    };
    table[0xB4] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_LDY_MEM,
    };
    table[0xAC] = Instruction {
        addr: ADDR_ABS,
        exec: OP_LDY_MEM,
    };
    table[0xBC] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_LDY_MEM,
    };

    // --- STA ---
    table[0x85] = Instruction {
        addr: ADDR_ZP,
        exec: OP_STA_MEM,
    };
    table[0x95] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_STA_MEM,
    };
    table[0x8D] = Instruction {
        addr: ADDR_ABS,
        exec: OP_STA_MEM,
    };
    table[0x9D] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_STA_MEM,
    };
    table[0x99] = Instruction {
        addr: ADDR_ABSY_WRITE,
        exec: OP_STA_MEM,
    };
    table[0x81] = Instruction {
        addr: ADDR_INDX,
        exec: OP_STA_MEM,
    };
    table[0x91] = Instruction {
        addr: ADDR_INDY_WRITE,
        exec: OP_STA_MEM,
    };

    // --- STX ---
    table[0x86] = Instruction {
        addr: ADDR_ZP,
        exec: OP_STX_MEM,
    };
    table[0x96] = Instruction {
        addr: ADDR_ZPY,
        exec: OP_STX_MEM,
    };
    table[0x8E] = Instruction {
        addr: ADDR_ABS,
        exec: OP_STX_MEM,
    };

    // --- STY ---
    table[0x84] = Instruction {
        addr: ADDR_ZP,
        exec: OP_STY_MEM,
    };
    table[0x94] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_STY_MEM,
    };
    table[0x8C] = Instruction {
        addr: ADDR_ABS,
        exec: OP_STY_MEM,
    };

    // --- ADC ---
    table[0x69] = Instruction {
        addr: ADDR_IMM,
        exec: OP_ADC_IMM,
    };
    table[0x65] = Instruction {
        addr: ADDR_ZP,
        exec: OP_ADC_MEM,
    };
    table[0x75] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_ADC_MEM,
    };
    table[0x6D] = Instruction {
        addr: ADDR_ABS,
        exec: OP_ADC_MEM,
    };
    table[0x7D] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_ADC_MEM,
    };
    table[0x79] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_ADC_MEM,
    };
    table[0x61] = Instruction {
        addr: ADDR_INDX,
        exec: OP_ADC_MEM,
    };
    table[0x71] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_ADC_MEM,
    };

    // --- SBC ---
    table[0xE9] = Instruction {
        addr: ADDR_IMM,
        exec: OP_SBC_IMM,
    };
    table[0xE5] = Instruction {
        addr: ADDR_ZP,
        exec: OP_SBC_MEM,
    };
    table[0xF5] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_SBC_MEM,
    };
    table[0xED] = Instruction {
        addr: ADDR_ABS,
        exec: OP_SBC_MEM,
    };
    table[0xFD] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_SBC_MEM,
    };
    table[0xF9] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_SBC_MEM,
    };
    table[0xE1] = Instruction {
        addr: ADDR_INDX,
        exec: OP_SBC_MEM,
    };
    table[0xF1] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_SBC_MEM,
    };

    // --- AND ---
    table[0x29] = Instruction {
        addr: ADDR_IMM,
        exec: OP_AND_IMM,
    };
    table[0x25] = Instruction {
        addr: ADDR_ZP,
        exec: OP_AND_MEM,
    };
    table[0x35] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_AND_MEM,
    };
    table[0x2D] = Instruction {
        addr: ADDR_ABS,
        exec: OP_AND_MEM,
    };
    table[0x3D] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_AND_MEM,
    };
    table[0x39] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_AND_MEM,
    };
    table[0x21] = Instruction {
        addr: ADDR_INDX,
        exec: OP_AND_MEM,
    };
    table[0x31] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_AND_MEM,
    };

    // --- ORA ---
    table[0x09] = Instruction {
        addr: ADDR_IMM,
        exec: OP_ORA_IMM,
    };
    table[0x05] = Instruction {
        addr: ADDR_ZP,
        exec: OP_ORA_MEM,
    };
    table[0x15] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_ORA_MEM,
    };
    table[0x0D] = Instruction {
        addr: ADDR_ABS,
        exec: OP_ORA_MEM,
    };
    table[0x1D] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_ORA_MEM,
    };
    table[0x19] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_ORA_MEM,
    };
    table[0x01] = Instruction {
        addr: ADDR_INDX,
        exec: OP_ORA_MEM,
    };
    table[0x11] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_ORA_MEM,
    };

    // --- EOR ---
    table[0x49] = Instruction {
        addr: ADDR_IMM,
        exec: OP_EOR_IMM,
    };
    table[0x45] = Instruction {
        addr: ADDR_ZP,
        exec: OP_EOR_MEM,
    };
    table[0x55] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_EOR_MEM,
    };
    table[0x4D] = Instruction {
        addr: ADDR_ABS,
        exec: OP_EOR_MEM,
    };
    table[0x5D] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_EOR_MEM,
    };
    table[0x59] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_EOR_MEM,
    };
    table[0x41] = Instruction {
        addr: ADDR_INDX,
        exec: OP_EOR_MEM,
    };
    table[0x51] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_EOR_MEM,
    };

    // --- BIT ---
    table[0x24] = Instruction {
        addr: ADDR_ZP,
        exec: OP_BIT_MEM,
    };
    table[0x2C] = Instruction {
        addr: ADDR_ABS,
        exec: OP_BIT_MEM,
    };

    // --- CMP ---
    table[0xC9] = Instruction {
        addr: ADDR_IMM,
        exec: OP_CMP_IMM,
    };
    table[0xC5] = Instruction {
        addr: ADDR_ZP,
        exec: OP_CMP_MEM,
    };
    table[0xD5] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_CMP_MEM,
    };
    table[0xCD] = Instruction {
        addr: ADDR_ABS,
        exec: OP_CMP_MEM,
    };
    table[0xDD] = Instruction {
        addr: ADDR_ABSX_READ,
        exec: OP_CMP_MEM,
    };
    table[0xD9] = Instruction {
        addr: ADDR_ABSY_READ,
        exec: OP_CMP_MEM,
    };
    table[0xC1] = Instruction {
        addr: ADDR_INDX,
        exec: OP_CMP_MEM,
    };
    table[0xD1] = Instruction {
        addr: ADDR_INDY_READ,
        exec: OP_CMP_MEM,
    };

    // --- CPX ---
    table[0xE0] = Instruction {
        addr: ADDR_IMM,
        exec: OP_CPX_IMM,
    };
    table[0xE4] = Instruction {
        addr: ADDR_ZP,
        exec: OP_CPX_MEM,
    };
    table[0xEC] = Instruction {
        addr: ADDR_ABS,
        exec: OP_CPX_MEM,
    };

    // --- CPY ---
    table[0xC0] = Instruction {
        addr: ADDR_IMM,
        exec: OP_CPY_IMM,
    };
    table[0xC4] = Instruction {
        addr: ADDR_ZP,
        exec: OP_CPY_MEM,
    };
    table[0xCC] = Instruction {
        addr: ADDR_ABS,
        exec: OP_CPY_MEM,
    };

    // --- ASL ---
    table[0x0A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_ASL_ACC,
    };
    table[0x06] = Instruction {
        addr: ADDR_ZP,
        exec: OP_ASL_MEM,
    };
    table[0x16] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_ASL_MEM,
    };
    table[0x0E] = Instruction {
        addr: ADDR_ABS,
        exec: OP_ASL_MEM,
    };
    table[0x1E] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_ASL_MEM,
    };

    // --- LSR ---
    table[0x4A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_LSR_ACC,
    };
    table[0x46] = Instruction {
        addr: ADDR_ZP,
        exec: OP_LSR_MEM,
    };
    table[0x56] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_LSR_MEM,
    };
    table[0x4E] = Instruction {
        addr: ADDR_ABS,
        exec: OP_LSR_MEM,
    };
    table[0x5E] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_LSR_MEM,
    };

    // --- ROL ---
    table[0x2A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_ROL_ACC,
    };
    table[0x26] = Instruction {
        addr: ADDR_ZP,
        exec: OP_ROL_MEM,
    };
    table[0x36] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_ROL_MEM,
    };
    table[0x2E] = Instruction {
        addr: ADDR_ABS,
        exec: OP_ROL_MEM,
    };
    table[0x3E] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_ROL_MEM,
    };

    // --- ROR ---
    table[0x6A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_ROR_ACC,
    };
    table[0x66] = Instruction {
        addr: ADDR_ZP,
        exec: OP_ROR_MEM,
    };
    table[0x76] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_ROR_MEM,
    };
    table[0x6E] = Instruction {
        addr: ADDR_ABS,
        exec: OP_ROR_MEM,
    };
    table[0x7E] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_ROR_MEM,
    };

    // --- INC ---
    table[0xE6] = Instruction {
        addr: ADDR_ZP,
        exec: OP_INC_MEM,
    };
    table[0xF6] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_INC_MEM,
    };
    table[0xEE] = Instruction {
        addr: ADDR_ABS,
        exec: OP_INC_MEM,
    };
    table[0xFE] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_INC_MEM,
    };

    // --- DEC ---
    table[0xC6] = Instruction {
        addr: ADDR_ZP,
        exec: OP_DEC_MEM,
    };
    table[0xD6] = Instruction {
        addr: ADDR_ZPX,
        exec: OP_DEC_MEM,
    };
    table[0xCE] = Instruction {
        addr: ADDR_ABS,
        exec: OP_DEC_MEM,
    };
    table[0xDE] = Instruction {
        addr: ADDR_ABSX_WRITE,
        exec: OP_DEC_MEM,
    };

    // --- Transf / Inc / Dec ---
    table[0xAA] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TAX,
    };
    table[0x8A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TXA,
    };
    table[0xA8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TAY,
    };
    table[0x98] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TYA,
    };
    table[0xBA] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TSX,
    };
    table[0x9A] = Instruction {
        addr: ADDR_NONE,
        exec: OP_TXS,
    };
    table[0xE8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_INX,
    };
    table[0xCA] = Instruction {
        addr: ADDR_NONE,
        exec: OP_DEX,
    };
    table[0xC8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_INY,
    };
    table[0x88] = Instruction {
        addr: ADDR_NONE,
        exec: OP_DEY,
    };

    // --- Flags ---
    table[0x18] = Instruction {
        addr: ADDR_NONE,
        exec: OP_CLC,
    };
    table[0x38] = Instruction {
        addr: ADDR_NONE,
        exec: OP_SEC,
    };
    table[0x58] = Instruction {
        addr: ADDR_NONE,
        exec: OP_CLI,
    };
    table[0x78] = Instruction {
        addr: ADDR_NONE,
        exec: OP_SEI,
    };
    table[0xB8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_CLV,
    };
    table[0xD8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_CLD,
    };
    table[0xF8] = Instruction {
        addr: ADDR_NONE,
        exec: OP_SED,
    };

    // --- Jump / Branch / Subroutine ---
    table[0x4C] = Instruction {
        addr: ADDR_ABS_JMP,
        exec: OP_NONE,
    };
    table[0x6C] = Instruction {
        addr: ADDR_ABS,
        exec: OP_JMP_IND,
    };
    table[0x20] = Instruction {
        addr: ADDR_NONE,
        exec: OP_JSR,
    };
    table[0x60] = Instruction {
        addr: ADDR_NONE,
        exec: OP_RTS,
    };
    table[0x40] = Instruction {
        addr: ADDR_NONE,
        exec: OP_RTI,
    };
    table[0x00] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRK,
    };

    table[0x10] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0x30] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0x50] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0x70] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0x90] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0xB0] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0xD0] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };
    table[0xF0] = Instruction {
        addr: ADDR_NONE,
        exec: OP_BRANCH,
    };

    // --- Stack ---
    table[0x08] = Instruction {
        addr: ADDR_NONE,
        exec: OP_PHP,
    };
    table[0x28] = Instruction {
        addr: ADDR_NONE,
        exec: OP_PLP,
    };
    table[0x48] = Instruction {
        addr: ADDR_NONE,
        exec: OP_PHA,
    };
    table[0x68] = Instruction {
        addr: ADDR_NONE,
        exec: OP_PLA,
    };

    // --- NOP ---
    table[0xEA] = Instruction {
        addr: ADDR_NONE,
        exec: OP_NOP,
    };

    table
}

#[link_section = ".data.pico1541_emulator_core"]
pub static INSTRUCTION_TABLE: [Instruction; 256] = init_table();
