#[cfg(feature = "debug")]

use crate::bus::Bus;
use crate::cpu::CPU;
use crate::instructions::nmos6502::*;
use core::fmt::Write;
use heapless::String;


/// Disassembles the instruction that was just executed by the CPU.
///
/// This function uses the debug state (`debug_pc`, `ir`, `debug_operand`)
/// to reconstruct the full instruction text.
#[cfg(feature = "debug")]
pub fn disassemble<B: Bus>(cpu: &CPU<B>) -> String<32> {
    format_instruction(cpu.ir, cpu.debug_pc, &cpu.debug_operand)
}

/// Disassembles an instruction at a specific address using a peek function.
#[cfg(feature = "debug")]
pub fn disassemble_at<F>(addr: u16, peek: F) -> (String<32>, u8) 
where F: Fn(u16) -> u8 {
    let opcode = peek(addr);
    let mut operands = [0u8; 2];
    let len = get_instruction_len(opcode);
    
    if len > 1 {
        operands[0] = peek(addr.wrapping_add(1));
    }
    if len > 2 {
        operands[1] = peek(addr.wrapping_add(2));
    }
    
    (format_instruction(opcode, addr, &operands), len)
}

#[cfg(feature = "debug")]
fn get_instruction_len(opcode: u8) -> u8 {
    let instruction = &INSTRUCTION_TABLE[opcode as usize];
    let addr_mode_ptr = instruction.addr.as_ptr();
    
    if addr_mode_ptr == ADDR_NONE.as_ptr() {
        1
    } else if addr_mode_ptr == ADDR_IMM.as_ptr() || 
              addr_mode_ptr == ADDR_ZP.as_ptr() ||
              addr_mode_ptr == ADDR_ZPX.as_ptr() ||
              addr_mode_ptr == ADDR_ZPY.as_ptr() ||
              addr_mode_ptr == ADDR_INDX.as_ptr() ||
              addr_mode_ptr == ADDR_INDY_READ.as_ptr() ||
              addr_mode_ptr == ADDR_INDY_WRITE.as_ptr() {
        2
    } else if addr_mode_ptr == ADDR_ABS.as_ptr() ||
              addr_mode_ptr == ADDR_ABS_JMP.as_ptr() ||
              addr_mode_ptr == ADDR_ABSX_READ.as_ptr() ||
              addr_mode_ptr == ADDR_ABSX_WRITE.as_ptr() ||
              addr_mode_ptr == ADDR_ABSY_READ.as_ptr() ||
              addr_mode_ptr == ADDR_ABSY_WRITE.as_ptr() {
        3
    } else {
        1
    }
}

#[cfg(feature = "debug")]
fn format_instruction(opcode: u8, pc: u16, operands: &[u8; 2]) -> String<32> {
    let mut s = String::<32>::new();
    let instruction = &INSTRUCTION_TABLE[opcode as usize];
    let mnemonic = instruction.mnemonic;
    let op1 = operands[0];
    let op2 = operands[1];
    let addr16 = u16::from_le_bytes([op1, op2]);

    let addr_mode_ptr = instruction.addr.as_ptr();
    let exec_mode_ptr = instruction.exec.as_ptr();

    if addr_mode_ptr == ADDR_NONE.as_ptr() {
        if exec_mode_ptr == OP_ASL_ACC.as_ptr() || exec_mode_ptr == OP_LSR_ACC.as_ptr() ||
           exec_mode_ptr == OP_ROL_ACC.as_ptr() || exec_mode_ptr == OP_ROR_ACC.as_ptr() {
            write!(s, "{} A", mnemonic).unwrap();
        } else if exec_mode_ptr == OP_BRANCH.as_ptr() {
            let offset = op1 as i8;
            let target_addr = pc.wrapping_add(2).wrapping_add(offset as u16);
            write!(s, "{} ${:04X}", mnemonic, target_addr).unwrap();
        } else {
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
            write!(s, "{} (${:04X})", mnemonic, addr16).unwrap();
        } else {
            write!(s, "{} ${:04X}", mnemonic, addr16).unwrap();
        }
    } else if addr_mode_ptr == ADDR_ABS_JMP.as_ptr() {
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
        write!(s, "{}", mnemonic).unwrap();
    }

    s
}
