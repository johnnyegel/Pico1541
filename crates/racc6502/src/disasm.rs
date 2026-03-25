#![cfg(debug_assertions)]

use crate::bus::Bus;
use crate::cpu::CPU;
use crate::instructions::nmos6502::*;
use core::fmt::Write;
use heapless::String;


/// Disassembles the instruction that was just executed by the CPU.
///
/// This function uses the debug state (`debug_pc`, `ir`, `debug_operand`)
/// to reconstruct the full instruction text.
///
/// # Arguments
///
/// * `cpu` - A reference to the CPU state *after* an instruction has finished.
///
/// # Returns
///
/// A `heapless::String` containing the formatted instruction, e.g., "LDA $1234,X".
pub fn disassemble<B: Bus>(cpu: &CPU<B>) -> String<32> {
    let mut s = String::<32>::new();
    let instruction = &INSTRUCTION_TABLE[cpu.ir as usize];
    let mnemonic = instruction.mnemonic;
    let op1 = if cpu.debug_operand_len > 0 { cpu.debug_operand[0] } else { 0 };
    let op2 = if cpu.debug_operand_len > 1 { cpu.debug_operand[1] } else { 0 };
    let addr16 = u16::from_le_bytes([op1, op2]);

    // Compare slice pointers to determine addressing mode. This is a zero-cost
    // way to check the mode without needing an enum in the instruction table.
    let addr_mode_ptr = instruction.addr.as_ptr();
    let exec_mode_ptr = instruction.exec.as_ptr();

    if addr_mode_ptr == ADDR_NONE.as_ptr() {
        if exec_mode_ptr == OP_ASL_ACC.as_ptr() || exec_mode_ptr == OP_LSR_ACC.as_ptr() ||
           exec_mode_ptr == OP_ROL_ACC.as_ptr() || exec_mode_ptr == OP_ROR_ACC.as_ptr() {
            // Accumulator mode
            write!(s, "{} A", mnemonic).unwrap();
        } else if exec_mode_ptr == OP_BRANCH.as_ptr() {
            // Relative mode
            let offset = op1 as i8;
            // PC for relative is from the instruction *after* the branch
            let target_addr = cpu.debug_pc.wrapping_add(2).wrapping_add(offset as u16);
            write!(s, "{} ${:04X}", mnemonic, target_addr).unwrap();
        } else {
            // Implied mode
            write!(s, "{}", mnemonic).unwrap();
        }
    } else if addr_mode_ptr == ADDR_IMM.as_ptr() {
        write!(s, "{} #${:02X}", mnemonic, op1).unwrap();
    } else if addr_mode_ptr == ADDR_ZP.as_ptr() {
        write!(s, "{} ${:02X}", mnemonic, op1).unwrap();
    } else if addr_mode_ptr == ADDR_ZPX.as_ptr() {
        write!(s, "{} ${:02X},X", mnemonic, op1).unwrap();
    } else if addr_mode_ptr == ADDR_ZPY.as_ptr() {
        write!(s, "{} ${:02X},Y", mnemonic, op1).unwrap();
    } else if addr_mode_ptr == ADDR_ABS.as_ptr() {
        if exec_mode_ptr == OP_JMP_IND.as_ptr() {
            // Indirect JMP
            write!(s, "{} (${:04X})", mnemonic, addr16).unwrap();
        } else {
            // Absolute
            write!(s, "{} ${:04X}", mnemonic, addr16).unwrap();
        }
    } else if addr_mode_ptr == ADDR_ABS_JMP.as_ptr() {
        // Absolute JMP
        write!(s, "{} ${:04X}", mnemonic, addr16).unwrap();
    } else if addr_mode_ptr == ADDR_ABSX_READ.as_ptr() || addr_mode_ptr == ADDR_ABSX_WRITE.as_ptr() {
        write!(s, "{} ${:04X},X", mnemonic, addr16).unwrap();
    } else if addr_mode_ptr == ADDR_ABSY_READ.as_ptr() || addr_mode_ptr == ADDR_ABSY_WRITE.as_ptr() {
        write!(s, "{} ${:04X},Y", mnemonic, addr16).unwrap();
    } else if addr_mode_ptr == ADDR_INDX.as_ptr() {
        write!(s, "{} (${:02X},X)", mnemonic, op1).unwrap();
    } else if addr_mode_ptr == ADDR_INDY_READ.as_ptr() || addr_mode_ptr == ADDR_INDY_WRITE.as_ptr() {
        write!(s, "{} (${:02X}),Y", mnemonic, op1).unwrap();
    } else {
        // Should not be reached for any legal opcode
        write!(s, "{}", mnemonic).unwrap();
    }

    s
}
