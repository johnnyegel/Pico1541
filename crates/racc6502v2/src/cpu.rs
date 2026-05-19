use crate::bus::Bus;
use crate::Opcode;

pub struct CPU {
    pub a: u8, pub x: u8, pub y: u8, pub s: u8, pub pc: u16, pub p: u8,
    pub opcode: u16, pub cycle: u8,
    pub effective_addr: u16, pub data_bus: u8, pub temp_addr: u16, pub page_crossed: bool,
    pub pending_interrupt: Option<Interrupt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Interrupt { Reset, Nmi, Irq, }
const OP_RESET: u16 = 0x100; const OP_NMI: u16 = 0x101; const OP_IRQ: u16 = 0x102;
const VECTOR_NMI: u16 = 0xFFFA; const VECTOR_RESET: u16 = 0xFFFC; const VECTOR_IRQ: u16 = 0xFFFE;

const BRK: u16 = Opcode::Brk as u16;
const ORA_INDX: u16 = Opcode::OraIndX as u16;
const ORA_ZP: u16 = Opcode::OraZp as u16;
const ASL_ZP: u16 = Opcode::AslZp as u16;
const PHP: u16 = Opcode::Php as u16;
const ORA_IMM: u16 = Opcode::OraImm as u16;
const ASL_ACC: u16 = Opcode::AslAcc as u16;
const ORA_ABS: u16 = Opcode::OraAbs as u16;
const ASL_ABS: u16 = Opcode::AslAbs as u16;

const BPL: u16 = Opcode::Bpl as u16;
const ORA_INDY: u16 = Opcode::OraIndY as u16;
const ORA_ZPX: u16 = Opcode::OraZpX as u16;
const ASL_ZPX: u16 = Opcode::AslZpX as u16;
const CLC: u16 = Opcode::Clc as u16;
const ORA_ABSY: u16 = Opcode::OraAbsY as u16;
const ORA_ABSX: u16 = Opcode::OraAbsX as u16;
const ASL_ABSX: u16 = Opcode::AslAbsX as u16;

const JSR: u16 = Opcode::Jsr as u16;
const AND_INDX: u16 = Opcode::AndIndX as u16;
const BIT_ZP: u16 = Opcode::BitZp as u16;
const AND_ZP: u16 = Opcode::AndZp as u16;
const ROL_ZP: u16 = Opcode::RolZp as u16;
const PLP: u16 = Opcode::Plp as u16;
const AND_IMM: u16 = Opcode::AndImm as u16;
const ROL_ACC: u16 = Opcode::RolAcc as u16;
const BIT_ABS: u16 = Opcode::BitAbs as u16;
const AND_ABS: u16 = Opcode::AndAbs as u16;
const ROL_ABS: u16 = Opcode::RolAbs as u16;

const BMI: u16 = Opcode::Bmi as u16;
const AND_INDY: u16 = Opcode::AndIndY as u16;
const AND_ZPX: u16 = Opcode::AndZpX as u16;
const ROL_ZPX: u16 = Opcode::RolZpX as u16;
const SEC: u16 = Opcode::Sec as u16;
const AND_ABSY: u16 = Opcode::AndAbsY as u16;
const AND_ABSX: u16 = Opcode::AndAbsX as u16;
const ROL_ABSX: u16 = Opcode::RolAbsX as u16;

const RTI: u16 = Opcode::Rti as u16;
const EOR_INDX: u16 = Opcode::EorIndX as u16;
const EOR_ZP: u16 = Opcode::EorZp as u16;
const LSR_ZP: u16 = Opcode::LsrZp as u16;
const PHA: u16 = Opcode::Pha as u16;
const EOR_IMM: u16 = Opcode::EorImm as u16;
const LSR_ACC: u16 = Opcode::LsrAcc as u16;
const JMP_ABS: u16 = Opcode::JmpAbs as u16;
const EOR_ABS: u16 = Opcode::EorAbs as u16;
const LSR_ABS: u16 = Opcode::LsrAbs as u16;

const BVC: u16 = Opcode::Bvc as u16;
const EOR_INDY: u16 = Opcode::EorIndY as u16;
const EOR_ZPX: u16 = Opcode::EorZpX as u16;
const LSR_ZPX: u16 = Opcode::LsrZpX as u16;
const CLI: u16 = Opcode::Cli as u16;
const EOR_ABSY: u16 = Opcode::EorAbsY as u16;
const EOR_ABSX: u16 = Opcode::EorAbsX as u16;
const LSR_ABSX: u16 = Opcode::LsrAbsX as u16;

const RTS: u16 = Opcode::Rts as u16;
const ADC_INDX: u16 = Opcode::AdcIndX as u16;
const ADC_ZP: u16 = Opcode::AdcZp as u16;
const ROR_ZP: u16 = Opcode::RorZp as u16;
const PLA: u16 = Opcode::Pla as u16;
const ADC_IMM: u16 = Opcode::AdcImm as u16;
const ROR_ACC: u16 = Opcode::RorAcc as u16;
const JMP_IND: u16 = Opcode::JmpInd as u16;
const ADC_ABS: u16 = Opcode::AdcAbs as u16;
const ROR_ABS: u16 = Opcode::RorAbs as u16;

const BVS: u16 = Opcode::Bvs as u16;
const ADC_INDY: u16 = Opcode::AdcIndY as u16;
const ADC_ZPX: u16 = Opcode::AdcZpX as u16;
const ROR_ZPX: u16 = Opcode::RorZpX as u16;
const SEI: u16 = Opcode::Sei as u16;
const ADC_ABSY: u16 = Opcode::AdcAbsY as u16;
const ADC_ABSX: u16 = Opcode::AdcAbsX as u16;
const ROR_ABSX: u16 = Opcode::RorAbsX as u16;

const STA_INDX: u16 = Opcode::StaIndX as u16;
const STY_ZP: u16 = Opcode::StyZp as u16;
const STA_ZP: u16 = Opcode::StaZp as u16;
const STX_ZP: u16 = Opcode::StxZp as u16;
const DEY: u16 = Opcode::Dey as u16;
const TXA: u16 = Opcode::Txa as u16;
const STY_ABS: u16 = Opcode::StyAbs as u16;
const STA_ABS: u16 = Opcode::StaAbs as u16;
const STX_ABS: u16 = Opcode::StxAbs as u16;

const BCC: u16 = Opcode::Bcc as u16;
const STA_INDY: u16 = Opcode::StaIndY as u16;
const STY_ZPX: u16 = Opcode::StyZpX as u16;
const STA_ZPX: u16 = Opcode::StaZpX as u16;
const STX_ZPY: u16 = Opcode::StxZpY as u16;
const TYA: u16 = Opcode::Tya as u16;
const STA_ABSY: u16 = Opcode::StaAbsY as u16;
const TXS: u16 = Opcode::Txs as u16;
const STA_ABSX: u16 = Opcode::StaAbsX as u16;

const LDY_IMM: u16 = Opcode::LdyImm as u16;
const LDA_INDX: u16 = Opcode::LdaIndX as u16;
const LDX_IMM: u16 = Opcode::LdxImm as u16;
const LDY_ZP: u16 = Opcode::LdyZp as u16;
const LDA_ZP: u16 = Opcode::LdaZp as u16;
const LDX_ZP: u16 = Opcode::LdxZp as u16;
const TAY: u16 = Opcode::Tay as u16;
const LDA_IMM: u16 = Opcode::LdaImm as u16;
const TAX: u16 = Opcode::Tax as u16;
const LDY_ABS: u16 = Opcode::LdyAbs as u16;
const LDA_ABS: u16 = Opcode::LdaAbs as u16;
const LDX_ABS: u16 = Opcode::LdxAbs as u16;

const BCS: u16 = Opcode::Bcs as u16;
const LDA_INDY: u16 = Opcode::LdaIndY as u16;
const LDY_ZPX: u16 = Opcode::LdyZpX as u16;
const LDA_ZPX: u16 = Opcode::LdaZpX as u16;
const LDX_ZPY: u16 = Opcode::LdxZpY as u16;
const CLV: u16 = Opcode::Clv as u16;
const LDA_ABSY: u16 = Opcode::LdaAbsY as u16;
const TSX: u16 = Opcode::Tsx as u16;
const LDY_ABSX: u16 = Opcode::LdyAbsX as u16;
const LDA_ABSX: u16 = Opcode::LdaAbsX as u16;
const LDX_ABSY: u16 = Opcode::LdxAbsY as u16;

const CPY_IMM: u16 = Opcode::CpyImm as u16;
const CMP_INDX: u16 = Opcode::CmpIndX as u16;
const CPY_ZP: u16 = Opcode::CpyZp as u16;
const CMP_ZP: u16 = Opcode::CmpZp as u16;
const DEC_ZP: u16 = Opcode::DecZp as u16;
const INY: u16 = Opcode::Iny as u16;
const CMP_IMM: u16 = Opcode::CmpImm as u16;
const DEX: u16 = Opcode::Dex as u16;
const CPY_ABS: u16 = Opcode::CpyAbs as u16;
const CMP_ABS: u16 = Opcode::CmpAbs as u16;
const DEC_ABS: u16 = Opcode::DecAbs as u16;

const BNE: u16 = Opcode::Bne as u16;
const CMP_INDY: u16 = Opcode::CmpIndY as u16;
const CMP_ZPX: u16 = Opcode::CmpZpX as u16;
const DEC_ZPX: u16 = Opcode::DecZpX as u16;
const CLD: u16 = Opcode::Cld as u16;
const CMP_ABSY: u16 = Opcode::CmpAbsY as u16;
const CMP_ABSX: u16 = Opcode::CmpAbsX as u16;
const DEC_ABSX: u16 = Opcode::DecAbsX as u16;

const CPX_IMM: u16 = Opcode::CpxImm as u16;
const SBC_INDX: u16 = Opcode::SbcIndX as u16;
const CPX_ZP: u16 = Opcode::CpxZp as u16;
const SBC_ZP: u16 = Opcode::SbcZp as u16;
const INC_ZP: u16 = Opcode::IncZp as u16;
const INX: u16 = Opcode::Inx as u16;
const SBC_IMM: u16 = Opcode::SbcImm as u16;
const NOP: u16 = Opcode::Nop as u16;
const CPX_ABS: u16 = Opcode::CpxAbs as u16;
const SBC_ABS: u16 = Opcode::SbcAbs as u16;
const INC_ABS: u16 = Opcode::IncAbs as u16;

const BEQ: u16 = Opcode::Beq as u16;
const SBC_INDY: u16 = Opcode::SbcIndY as u16;
const SBC_ZPX: u16 = Opcode::SbcZpX as u16;
const INC_ZPX: u16 = Opcode::IncZpX as u16;
const SED: u16 = Opcode::Sed as u16;
const SBC_ABSY: u16 = Opcode::SbcAbsY as u16;
const SBC_ABSX: u16 = Opcode::SbcAbsX as u16;
const INC_ABSX: u16 = Opcode::IncAbsX as u16;

impl CPU {
    pub fn new() -> Self { Self { a: 0, x: 0, y: 0, s: 0xFD, pc: 0, p: 0x24, opcode: 0, cycle: 0, effective_addr: 0, data_bus: 0, temp_addr: 0, page_crossed: false, pending_interrupt: None, } }
    pub fn reset(&mut self) { self.pending_interrupt = Some(Interrupt::Reset); self.cycle = 0; }

    #[inline(always)]
    pub fn tick<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if bus.so_pending() { self.p |= 0x40; }
        match (self.opcode, self.cycle) {
            (_, 0) => self.fetch_opcode(bus, ctx),
            (OP_RESET, 1) => self.reset_c1(bus, ctx), (OP_RESET, 2) => self.reset_c2(bus, ctx), (OP_RESET, 3) => self.reset_c3(bus, ctx), (OP_RESET, 4) => self.reset_c4(bus, ctx), (OP_RESET, 5) => self.reset_c5(bus, ctx), (OP_RESET, 6) => self.reset_c6(bus, ctx),
            (OP_NMI, 1) => self.int_c1(bus, ctx), (OP_NMI, 2) => self.int_c2(bus, ctx), (OP_NMI, 3) => self.int_c3(bus, ctx), (OP_NMI, 4) => self.int_c4(bus, ctx), (OP_NMI, 5) => self.int_c5(bus, ctx, VECTOR_NMI), (OP_NMI, 6) => self.int_c6(bus, ctx, VECTOR_NMI),
            (OP_IRQ, 1) => self.int_c1(bus, ctx), (OP_IRQ, 2) => self.int_c2(bus, ctx), (OP_IRQ, 3) => self.int_c3(bus, ctx), (OP_IRQ, 4) => self.int_c4(bus, ctx), (OP_IRQ, 5) => self.int_c5(bus, ctx, VECTOR_IRQ), (OP_IRQ, 6) => self.int_c6(bus, ctx, VECTOR_IRQ),
            (BRK, 1) => self.brk_c1(bus, ctx), (BRK, 2) => self.brk_c2(bus, ctx), (BRK, 3) => self.brk_c3(bus, ctx), (BRK, 4) => self.brk_c4(bus, ctx), (BRK, 5) => self.int_c5(bus, ctx, VECTOR_IRQ), (BRK, 6) => self.int_c6(bus, ctx, VECTOR_IRQ),

            // --- LDA ---
            (LDA_IMM, 1) => self.lda_imm_c1(bus, ctx),
            (LDA_ZP, 1) => self.ld_zp_c1(bus, ctx), (LDA_ZP, 2) => self.lda_zp_c2(bus, ctx),
            (LDA_ZPX, 1) => self.ld_zp_c1(bus, ctx), (LDA_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (LDA_ZPX, 3) => self.lda_zp_c2(bus, ctx),
            (LDA_ABS, 1) => self.ld_abs_c1(bus, ctx), (LDA_ABS, 2) => self.ld_abs_c2(bus, ctx), (LDA_ABS, 3) => self.lda_abs_c3(bus, ctx),
            (LDA_ABSX, 1) => self.ld_abs_c1(bus, ctx), (LDA_ABSX, 2) => self.ld_absx_c2(bus, ctx), (LDA_ABSX, 3) => self.lda_absx_c3(bus, ctx), (LDA_ABSX, 4) => self.lda_absx_c4(bus, ctx),
            (LDA_ABSY, 1) => self.ld_abs_c1(bus, ctx), (LDA_ABSY, 2) => self.ld_absy_c2(bus, ctx), (LDA_ABSY, 3) => self.lda_absy_c3(bus, ctx), (LDA_ABSY, 4) => self.lda_absy_c4(bus, ctx),
            (LDA_INDX, 1) => self.ld_zp_c1(bus, ctx), (LDA_INDX, 2) => self.ld_indx_c2(bus, ctx), (LDA_INDX, 3) => self.ld_indx_c3(bus, ctx), (LDA_INDX, 4) => self.ld_indx_c4(bus, ctx), (LDA_INDX, 5) => self.lda_zp_c2(bus, ctx),
            (LDA_INDY, 1) => self.ld_zp_c1(bus, ctx), (LDA_INDY, 2) => self.ld_indy_c2(bus, ctx), (LDA_INDY, 3) => self.ld_indy_c3(bus, ctx), (LDA_INDY, 4) => self.lda_indy_c4(bus, ctx), (LDA_INDY, 5) => self.lda_indy_c5(bus, ctx),

            // --- LDX ---
            (LDX_IMM, 1) => self.ldx_imm_c1(bus, ctx),
            (LDX_ZP, 1) => self.ld_zp_c1(bus, ctx), (LDX_ZP, 2) => self.ldx_zp_c2(bus, ctx),
            (LDX_ZPY, 1) => self.ld_zp_c1(bus, ctx), (LDX_ZPY, 2) => self.ld_zpy_c2(bus, ctx), (LDX_ZPY, 3) => self.ldx_zp_c2(bus, ctx),
            (LDX_ABS, 1) => self.ld_abs_c1(bus, ctx), (LDX_ABS, 2) => self.ld_abs_c2(bus, ctx), (LDX_ABS, 3) => self.ldx_abs_c3(bus, ctx),
            (LDX_ABSY, 1) => self.ld_abs_c1(bus, ctx), (LDX_ABSY, 2) => self.ld_absy_c2(bus, ctx), (LDX_ABSY, 3) => self.ldx_absy_c3(bus, ctx), (LDX_ABSY, 4) => self.ldx_absy_c4(bus, ctx),

            // --- LDY ---
            (LDY_IMM, 1) => self.ldy_imm_c1(bus, ctx),
            (LDY_ZP, 1) => self.ld_zp_c1(bus, ctx), (LDY_ZP, 2) => self.ldy_zp_c2(bus, ctx),
            (LDY_ZPX, 1) => self.ld_zp_c1(bus, ctx), (LDY_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (LDY_ZPX, 3) => self.ldy_zp_c2(bus, ctx),
            (LDY_ABS, 1) => self.ld_abs_c1(bus, ctx), (LDY_ABS, 2) => self.ld_abs_c2(bus, ctx), (LDY_ABS, 3) => self.ldy_abs_c3(bus, ctx),
            (LDY_ABSX, 1) => self.ld_abs_c1(bus, ctx), (LDY_ABSX, 2) => self.ld_absx_c2(bus, ctx), (LDY_ABSX, 3) => self.ldy_absx_c3(bus, ctx), (LDY_ABSX, 4) => self.ldy_absx_c4(bus, ctx),

            // --- STA ---
            (STA_ZP, 1) => self.ld_zp_c1(bus, ctx), (STA_ZP, 2) => self.sta_zp_c2(bus, ctx),
            (STA_ZPX, 1) => self.ld_zp_c1(bus, ctx), (STA_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (STA_ZPX, 3) => self.sta_zp_c2(bus, ctx),
            (STA_ABS, 1) => self.ld_abs_c1(bus, ctx), (STA_ABS, 2) => self.ld_abs_c2(bus, ctx), (STA_ABS, 3) => self.sta_abs_c3(bus, ctx),
            (STA_ABSX, 1) => self.ld_abs_c1(bus, ctx), (STA_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (STA_ABSX, 3) => self.sta_abs_c3(bus, ctx),
            (STA_ABSY, 1) => self.ld_abs_c1(bus, ctx), (STA_ABSY, 2) => self.ld_absy_write_c2(bus, ctx), (STA_ABSY, 3) => self.sta_abs_c3(bus, ctx),
            (STA_INDX, 1) => self.ld_zp_c1(bus, ctx), (STA_INDX, 2) => self.ld_indx_c2(bus, ctx), (STA_INDX, 3) => self.ld_indx_c3(bus, ctx), (STA_INDX, 4) => self.ld_indx_c4(bus, ctx), (STA_INDX, 5) => self.sta_zp_c2(bus, ctx),
            (STA_INDY, 1) => self.ld_zp_c1(bus, ctx), (STA_INDY, 2) => self.ld_indy_c2(bus, ctx), (STA_INDY, 3) => self.ld_indy_write_c3(bus, ctx), (STA_INDY, 4) => self.sta_zp_c2(bus, ctx),

            // --- STX/STY ---
            (STX_ZP, 1) => self.ld_zp_c1(bus, ctx), (STX_ZP, 2) => self.stx_zp_c2(bus, ctx),
            (STX_ZPY, 1) => self.ld_zp_c1(bus, ctx), (STX_ZPY, 2) => self.ld_zpy_c2(bus, ctx), (STX_ZPY, 3) => self.stx_zp_c2(bus, ctx),
            (STX_ABS, 1) => self.ld_abs_c1(bus, ctx), (STX_ABS, 2) => self.ld_abs_c2(bus, ctx), (STX_ABS, 3) => self.stx_abs_c3(bus, ctx),
            (STY_ZP, 1) => self.ld_zp_c1(bus, ctx), (STY_ZP, 2) => self.sty_zp_c2(bus, ctx),
            (STY_ZPX, 1) => self.ld_zp_c1(bus, ctx), (STY_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (STY_ZPX, 3) => self.sty_zp_c2(bus, ctx),
            (STY_ABS, 1) => self.ld_abs_c1(bus, ctx), (STY_ABS, 2) => self.ld_abs_c2(bus, ctx), (STY_ABS, 3) => self.sty_abs_c3(bus, ctx),

            // --- Transfers ---
            (TAX, 1) => self.tax_c1(bus, ctx), (TXA, 1) => self.txa_c1(bus, ctx), (TAY, 1) => self.tay_c1(bus, ctx), (TYA, 1) => self.tya_c1(bus, ctx), (TSX, 1) => self.tsx_c1(bus, ctx), (TXS, 1) => self.txs_c1(bus, ctx),

            // --- ALU ---
            (ADC_IMM, 1) => self.adc_imm_c1(bus, ctx), (ADC_ZP, 1) => self.ld_zp_c1(bus, ctx), (ADC_ZP, 2) => self.adc_zp_c2(bus, ctx), (ADC_ZPX, 1) => self.ld_zp_c1(bus, ctx), (ADC_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (ADC_ZPX, 3) => self.adc_zp_c2(bus, ctx),
            (ADC_ABS, 1) => self.ld_abs_c1(bus, ctx), (ADC_ABS, 2) => self.ld_abs_c2(bus, ctx), (ADC_ABS, 3) => self.adc_abs_c3(bus, ctx),
            (ADC_ABSX, 1) => self.ld_abs_c1(bus, ctx), (ADC_ABSX, 2) => self.ld_absx_c2(bus, ctx), (ADC_ABSX, 3) => self.adc_absx_c3(bus, ctx), (ADC_ABSX, 4) => self.adc_absx_c4(bus, ctx),
            (ADC_ABSY, 1) => self.ld_abs_c1(bus, ctx), (ADC_ABSY, 2) => self.ld_absy_c2(bus, ctx), (ADC_ABSY, 3) => self.adc_absy_c3(bus, ctx), (ADC_ABSY, 4) => self.adc_absy_c4(bus, ctx),
            (ADC_INDX, 1) => self.ld_zp_c1(bus, ctx), (ADC_INDX, 2) => self.ld_indx_c2(bus, ctx), (ADC_INDX, 3) => self.ld_indx_c3(bus, ctx), (ADC_INDX, 4) => self.ld_indx_c4(bus, ctx), (ADC_INDX, 5) => self.adc_zp_c2(bus, ctx),
            (ADC_INDY, 1) => self.ld_zp_c1(bus, ctx), (ADC_INDY, 2) => self.ld_indy_c2(bus, ctx), (ADC_INDY, 3) => self.ld_indy_c3(bus, ctx), (ADC_INDY, 4) => self.adc_indy_c4(bus, ctx), (ADC_INDY, 5) => self.adc_indy_c5(bus, ctx),
            (SBC_IMM, 1) => self.sbc_imm_c1(bus, ctx), (SBC_ZP, 1) => self.ld_zp_c1(bus, ctx), (SBC_ZP, 2) => self.sbc_zp_c2(bus, ctx), (SBC_ZPX, 1) => self.ld_zp_c1(bus, ctx), (SBC_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (SBC_ZPX, 3) => self.sbc_zp_c2(bus, ctx),
            (SBC_ABS, 1) => self.ld_abs_c1(bus, ctx), (SBC_ABS, 2) => self.ld_abs_c2(bus, ctx), (SBC_ABS, 3) => self.sbc_abs_c3(bus, ctx),
            (SBC_ABSX, 1) => self.ld_abs_c1(bus, ctx), (SBC_ABSX, 2) => self.ld_absx_c2(bus, ctx), (SBC_ABSX, 3) => self.sbc_absx_c3(bus, ctx), (SBC_ABSX, 4) => self.sbc_absx_c4(bus, ctx),
            (SBC_ABSY, 1) => self.ld_abs_c1(bus, ctx), (SBC_ABSY, 2) => self.ld_absy_c2(bus, ctx), (SBC_ABSY, 3) => self.sbc_absy_c3(bus, ctx), (SBC_ABSY, 4) => self.sbc_absy_c4(bus, ctx),
            (SBC_INDX, 1) => self.ld_zp_c1(bus, ctx), (SBC_INDX, 2) => self.ld_indx_c2(bus, ctx), (SBC_INDX, 3) => self.ld_indx_c3(bus, ctx), (SBC_INDX, 4) => self.ld_indx_c4(bus, ctx), (SBC_INDX, 5) => self.sbc_zp_c2(bus, ctx),
            (SBC_INDY, 1) => self.ld_zp_c1(bus, ctx), (SBC_INDY, 2) => self.ld_indy_c2(bus, ctx), (SBC_INDY, 3) => self.ld_indy_c3(bus, ctx), (SBC_INDY, 4) => self.sbc_indy_c4(bus, ctx), (SBC_INDY, 5) => self.sbc_indy_c5(bus, ctx),
            (AND_IMM, 1) => self.and_imm_c1(bus, ctx), (AND_ZP, 1) => self.ld_zp_c1(bus, ctx), (AND_ZP, 2) => self.and_zp_c2(bus, ctx), (AND_ZPX, 1) => self.ld_zp_c1(bus, ctx), (AND_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (AND_ZPX, 3) => self.and_zp_c2(bus, ctx),
            (AND_ABS, 1) => self.ld_abs_c1(bus, ctx), (AND_ABS, 2) => self.ld_abs_c2(bus, ctx), (AND_ABS, 3) => self.and_abs_c3(bus, ctx),
            (AND_ABSX, 1) => self.ld_abs_c1(bus, ctx), (AND_ABSX, 2) => self.ld_absx_c2(bus, ctx), (AND_ABSX, 3) => self.and_absx_c3(bus, ctx), (AND_ABSX, 4) => self.and_absx_c4(bus, ctx),
            (AND_ABSY, 1) => self.ld_abs_c1(bus, ctx), (AND_ABSY, 2) => self.ld_absy_c2(bus, ctx), (AND_ABSY, 3) => self.and_absy_c3(bus, ctx), (AND_ABSY, 4) => self.and_absy_c4(bus, ctx),
            (AND_INDX, 1) => self.ld_zp_c1(bus, ctx), (AND_INDX, 2) => self.ld_indx_c2(bus, ctx), (AND_INDX, 3) => self.ld_indx_c3(bus, ctx), (AND_INDX, 4) => self.ld_indx_c4(bus, ctx), (AND_INDX, 5) => self.and_zp_c2(bus, ctx),
            (AND_INDY, 1) => self.ld_zp_c1(bus, ctx), (AND_INDY, 2) => self.ld_indy_c2(bus, ctx), (AND_INDY, 3) => self.ld_indy_c3(bus, ctx), (AND_INDY, 4) => self.and_indy_c4(bus, ctx), (AND_INDY, 5) => self.and_indy_c5(bus, ctx),
            (ORA_IMM, 1) => self.ora_imm_c1(bus, ctx), (ORA_ZP, 1) => self.ld_zp_c1(bus, ctx), (ORA_ZP, 2) => self.ora_zp_c2(bus, ctx), (ORA_ZPX, 1) => self.ld_zp_c1(bus, ctx), (ORA_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (ORA_ZPX, 3) => self.ora_zp_c2(bus, ctx),
            (ORA_ABS, 1) => self.ld_abs_c1(bus, ctx), (ORA_ABS, 2) => self.ld_abs_c2(bus, ctx), (ORA_ABS, 3) => self.ora_abs_c3(bus, ctx),
            (ORA_ABSX, 1) => self.ld_abs_c1(bus, ctx), (ORA_ABSX, 2) => self.ld_absx_c2(bus, ctx), (ORA_ABSX, 3) => self.ora_absx_c3(bus, ctx), (ORA_ABSX, 4) => self.ora_absx_c4(bus, ctx),
            (ORA_ABSY, 1) => self.ld_abs_c1(bus, ctx), (ORA_ABSY, 2) => self.ld_absy_c2(bus, ctx), (ORA_ABSY, 3) => self.ora_absy_c3(bus, ctx), (ORA_ABSY, 4) => self.ora_absy_c4(bus, ctx),
            (ORA_INDX, 1) => self.ld_zp_c1(bus, ctx), (ORA_INDX, 2) => self.ld_indx_c2(bus, ctx), (ORA_INDX, 3) => self.ld_indx_c3(bus, ctx), (ORA_INDX, 4) => self.ld_indx_c4(bus, ctx), (ORA_INDX, 5) => self.ora_zp_c2(bus, ctx),
            (ORA_INDY, 1) => self.ld_zp_c1(bus, ctx), (ORA_INDY, 2) => self.ld_indy_c2(bus, ctx), (ORA_INDY, 3) => self.ld_indy_c3(bus, ctx), (ORA_INDY, 4) => self.ora_indy_c4(bus, ctx), (ORA_INDY, 5) => self.ora_indy_c5(bus, ctx),
            (EOR_IMM, 1) => self.eor_imm_c1(bus, ctx), (EOR_ZP, 1) => self.ld_zp_c1(bus, ctx), (EOR_ZP, 2) => self.eor_zp_c2(bus, ctx), (EOR_ZPX, 1) => self.ld_zp_c1(bus, ctx), (EOR_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (EOR_ZPX, 3) => self.eor_zp_c2(bus, ctx),
            (EOR_ABS, 1) => self.ld_abs_c1(bus, ctx), (EOR_ABS, 2) => self.ld_abs_c2(bus, ctx), (EOR_ABS, 3) => self.eor_abs_c3(bus, ctx),
            (EOR_ABSX, 1) => self.ld_abs_c1(bus, ctx), (EOR_ABSX, 2) => self.ld_absx_c2(bus, ctx), (EOR_ABSX, 3) => self.eor_absx_c3(bus, ctx), (EOR_ABSX, 4) => self.eor_absx_c4(bus, ctx),
            (EOR_ABSY, 1) => self.ld_abs_c1(bus, ctx), (EOR_ABSY, 2) => self.ld_absy_c2(bus, ctx), (EOR_ABSY, 3) => self.eor_absy_c3(bus, ctx), (EOR_ABSY, 4) => self.eor_absy_c4(bus, ctx),
            (EOR_INDX, 1) => self.ld_zp_c1(bus, ctx), (EOR_INDX, 2) => self.ld_indx_c2(bus, ctx), (EOR_INDX, 3) => self.ld_indx_c3(bus, ctx), (EOR_INDX, 4) => self.ld_indx_c4(bus, ctx), (EOR_INDX, 5) => self.eor_zp_c2(bus, ctx),
            (EOR_INDY, 1) => self.ld_zp_c1(bus, ctx), (EOR_INDY, 2) => self.ld_indy_c2(bus, ctx), (EOR_INDY, 3) => self.ld_indy_c3(bus, ctx), (EOR_INDY, 4) => self.eor_indy_c4(bus, ctx), (EOR_INDY, 5) => self.eor_indy_c5(bus, ctx),
            (CMP_IMM, 1) => self.cmp_imm_c1(bus, ctx), (CMP_ZP, 1) => self.ld_zp_c1(bus, ctx), (CMP_ZP, 2) => self.cmp_zp_c2(bus, ctx), (CMP_ZPX, 1) => self.ld_zp_c1(bus, ctx), (CMP_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (CMP_ZPX, 3) => self.cmp_zp_c2(bus, ctx),
            (CMP_ABS, 1) => self.ld_abs_c1(bus, ctx), (CMP_ABS, 2) => self.ld_abs_c2(bus, ctx), (CMP_ABS, 3) => self.cmp_abs_c3(bus, ctx),
            (CMP_ABSX, 1) => self.ld_abs_c1(bus, ctx), (CMP_ABSX, 2) => self.ld_absx_c2(bus, ctx), (CMP_ABSX, 3) => self.cmp_absx_c3(bus, ctx), (CMP_ABSX, 4) => self.cmp_absx_c4(bus, ctx),
            (CMP_ABSY, 1) => self.ld_abs_c1(bus, ctx), (CMP_ABSY, 2) => self.ld_absy_c2(bus, ctx), (CMP_ABSY, 3) => self.cmp_absy_c3(bus, ctx), (CMP_ABSY, 4) => self.cmp_absy_c4(bus, ctx),
            (CMP_INDX, 1) => self.ld_zp_c1(bus, ctx), (CMP_INDX, 2) => self.ld_indx_c2(bus, ctx), (CMP_INDX, 3) => self.ld_indx_c3(bus, ctx), (CMP_INDX, 4) => self.ld_indx_c4(bus, ctx), (CMP_INDX, 5) => self.cmp_zp_c2(bus, ctx),
            (CMP_INDY, 1) => self.ld_zp_c1(bus, ctx), (CMP_INDY, 2) => self.ld_indy_c2(bus, ctx), (CMP_INDY, 3) => self.ld_indy_c3(bus, ctx), (CMP_INDY, 4) => self.cmp_indy_c4(bus, ctx), (CMP_INDY, 5) => self.cmp_indy_c5(bus, ctx),
            (CPX_IMM, 1) => self.cpx_imm_c1(bus, ctx), (CPX_ZP, 1) => self.ld_zp_c1(bus, ctx), (CPX_ZP, 2) => self.cpx_zp_c2(bus, ctx), (CPX_ABS, 1) => self.ld_abs_c1(bus, ctx), (CPX_ABS, 2) => self.ld_abs_c2(bus, ctx), (CPX_ABS, 3) => self.cpx_abs_c3(bus, ctx),
            (CPY_IMM, 1) => self.cpy_imm_c1(bus, ctx), (CPY_ZP, 1) => self.ld_zp_c1(bus, ctx), (CPY_ZP, 2) => self.cpy_zp_c2(bus, ctx), (CPY_ABS, 1) => self.ld_abs_c1(bus, ctx), (CPY_ABS, 2) => self.ld_abs_c2(bus, ctx), (CPY_ABS, 3) => self.cpy_abs_c3(bus, ctx),
            (BIT_ZP, 1) => self.ld_zp_c1(bus, ctx), (BIT_ZP, 2) => self.bit_zp_c2(bus, ctx), (BIT_ABS, 1) => self.ld_abs_c1(bus, ctx), (BIT_ABS, 2) => self.ld_abs_c2(bus, ctx), (BIT_ABS, 3) => self.bit_abs_c3(bus, ctx),

            // --- Shifts/Rotates ---
            (ASL_ACC, 1) => self.asl_acc_c1(bus, ctx), (ASL_ZP, 1) => self.ld_zp_c1(bus, ctx), (ASL_ZP, 2) => self.rmw_zp_c2(bus, ctx), (ASL_ZP, 3) => self.rmw_zp_c3(bus, ctx), (ASL_ZP, 4) => self.asl_zp_c4(bus, ctx),
            (ASL_ZPX, 1) => self.ld_zp_c1(bus, ctx), (ASL_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (ASL_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (ASL_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (ASL_ZPX, 5) => self.asl_zp_c5(bus, ctx),
            (ASL_ABS, 1) => self.ld_abs_c1(bus, ctx), (ASL_ABS, 2) => self.ld_abs_c2(bus, ctx), (ASL_ABS, 3) => self.rmw_abs_c3(bus, ctx), (ASL_ABS, 4) => self.rmw_abs_c4(bus, ctx), (ASL_ABS, 5) => self.asl_abs_c5(bus, ctx),
            (ASL_ABSX, 1) => self.ld_abs_c1(bus, ctx), (ASL_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (ASL_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (ASL_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (ASL_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (ASL_ABSX, 6) => self.asl_abs_c6(bus, ctx),
            (LSR_ACC, 1) => self.lsr_acc_c1(bus, ctx), (LSR_ZP, 1) => self.ld_zp_c1(bus, ctx), (LSR_ZP, 2) => self.rmw_zp_c2(bus, ctx), (LSR_ZP, 3) => self.rmw_zp_c3(bus, ctx), (LSR_ZP, 4) => self.lsr_zp_c4(bus, ctx),
            (LSR_ZPX, 1) => self.ld_zp_c1(bus, ctx), (LSR_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (LSR_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (LSR_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (LSR_ZPX, 5) => self.lsr_zp_c5(bus, ctx),
            (LSR_ABS, 1) => self.ld_abs_c1(bus, ctx), (LSR_ABS, 2) => self.ld_abs_c2(bus, ctx), (LSR_ABS, 3) => self.rmw_abs_c3(bus, ctx), (LSR_ABS, 4) => self.rmw_abs_c4(bus, ctx), (LSR_ABS, 5) => self.lsr_abs_c5(bus, ctx),
            (LSR_ABSX, 1) => self.ld_abs_c1(bus, ctx), (LSR_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (LSR_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (LSR_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (LSR_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (LSR_ABSX, 6) => self.lsr_abs_c6(bus, ctx),
            (ROL_ACC, 1) => self.rol_acc_c1(bus, ctx), (ROL_ZP, 1) => self.ld_zp_c1(bus, ctx), (ROL_ZP, 2) => self.rmw_zp_c2(bus, ctx), (ROL_ZP, 3) => self.rmw_zp_c3(bus, ctx), (ROL_ZP, 4) => self.rol_zp_c4(bus, ctx),
            (ROL_ZPX, 1) => self.ld_zp_c1(bus, ctx), (ROL_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (ROL_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (ROL_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (ROL_ZPX, 5) => self.rol_zp_c5(bus, ctx),
            (ROL_ABS, 1) => self.ld_abs_c1(bus, ctx), (ROL_ABS, 2) => self.ld_abs_c2(bus, ctx), (ROL_ABS, 3) => self.rmw_abs_c3(bus, ctx), (ROL_ABS, 4) => self.rmw_abs_c4(bus, ctx), (ROL_ABS, 5) => self.rol_abs_c5(bus, ctx),
            (ROL_ABSX, 1) => self.ld_abs_c1(bus, ctx), (ROL_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (ROL_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (ROL_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (ROL_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (ROL_ABSX, 6) => self.rol_abs_c6(bus, ctx),
            (ROR_ACC, 1) => self.ror_acc_c1(bus, ctx), (ROR_ZP, 1) => self.ld_zp_c1(bus, ctx), (ROR_ZP, 2) => self.rmw_zp_c2(bus, ctx), (ROR_ZP, 3) => self.rmw_zp_c3(bus, ctx), (ROR_ZP, 4) => self.ror_zp_c4(bus, ctx),
            (ROR_ZPX, 1) => self.ld_zp_c1(bus, ctx), (ROR_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (ROR_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (ROR_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (ROR_ZPX, 5) => self.ror_zp_c5(bus, ctx),
            (ROR_ABS, 1) => self.ld_abs_c1(bus, ctx), (ROR_ABS, 2) => self.ld_abs_c2(bus, ctx), (ROR_ABS, 3) => self.rmw_abs_c3(bus, ctx), (ROR_ABS, 4) => self.rmw_abs_c4(bus, ctx), (ROR_ABS, 5) => self.ror_abs_c5(bus, ctx),
            (ROR_ABSX, 1) => self.ld_abs_c1(bus, ctx), (ROR_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (ROR_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (ROR_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (ROR_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (ROR_ABSX, 6) => self.ror_abs_c6(bus, ctx),

            // --- Increment/Decrement (Batch 4) ---
            (INC_ZP, 1) => self.ld_zp_c1(bus, ctx), (INC_ZP, 2) => self.rmw_zp_c2(bus, ctx), (INC_ZP, 3) => self.rmw_zp_c3(bus, ctx), (INC_ZP, 4) => self.inc_zp_c4(bus, ctx),
            (INC_ZPX, 1) => self.ld_zp_c1(bus, ctx), (INC_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (INC_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (INC_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (INC_ZPX, 5) => self.inc_zp_c5(bus, ctx),
            (INC_ABS, 1) => self.ld_abs_c1(bus, ctx), (INC_ABS, 2) => self.ld_abs_c2(bus, ctx), (INC_ABS, 3) => self.rmw_abs_c3(bus, ctx), (INC_ABS, 4) => self.rmw_abs_c4(bus, ctx), (INC_ABS, 5) => self.inc_abs_c5(bus, ctx),
            (INC_ABSX, 1) => self.ld_abs_c1(bus, ctx), (INC_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (INC_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (INC_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (INC_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (INC_ABSX, 6) => self.inc_abs_c6(bus, ctx),
            (DEC_ZP, 1) => self.ld_zp_c1(bus, ctx), (DEC_ZP, 2) => self.rmw_zp_c2(bus, ctx), (DEC_ZP, 3) => self.rmw_zp_c3(bus, ctx), (DEC_ZP, 4) => self.dec_zp_c4(bus, ctx),
            (DEC_ZPX, 1) => self.ld_zp_c1(bus, ctx), (DEC_ZPX, 2) => self.ld_zpx_c2(bus, ctx), (DEC_ZPX, 3) => self.rmw_zp_c3_read(bus, ctx), (DEC_ZPX, 4) => self.rmw_zp_c4(bus, ctx), (DEC_ZPX, 5) => self.dec_zp_c5(bus, ctx),
            (DEC_ABS, 1) => self.ld_abs_c1(bus, ctx), (DEC_ABS, 2) => self.ld_abs_c2(bus, ctx), (DEC_ABS, 3) => self.rmw_abs_c3(bus, ctx), (DEC_ABS, 4) => self.rmw_abs_c4(bus, ctx), (DEC_ABS, 5) => self.dec_abs_c5(bus, ctx),
            (DEC_ABSX, 1) => self.ld_abs_c1(bus, ctx), (DEC_ABSX, 2) => self.ld_absx_write_c2(bus, ctx), (DEC_ABSX, 3) => self.rmw_abs_c3(bus, ctx), (DEC_ABSX, 4) => self.rmw_abs_c4(bus, ctx), (DEC_ABSX, 5) => self.rmw_abs_c5(bus, ctx), (DEC_ABSX, 6) => self.dec_abs_c6(bus, ctx),
            (INX, 1) => self.inx_c1(bus, ctx), (INY, 1) => self.iny_c1(bus, ctx), (DEX, 1) => self.dex_c1(bus, ctx), (DEY, 1) => self.dey_c1(bus, ctx),

            // --- Jumps/Calls (Batch 5) ---
            (JMP_ABS, 1) => self.jmp_abs_c1(bus, ctx), (JMP_ABS, 2) => self.jmp_abs_c2(bus, ctx),
            (JMP_IND, 1) => self.jmp_ind_c1(bus, ctx), (JMP_IND, 2) => self.jmp_ind_c2(bus, ctx), (JMP_IND, 3) => self.jmp_ind_c3(bus, ctx), (JMP_IND, 4) => self.jmp_ind_c4(bus, ctx),
            (JSR, 1) => self.jsr_c1(bus, ctx), (JSR, 2) => self.jsr_c2(bus, ctx), (JSR, 3) => self.jsr_c3(bus, ctx), (JSR, 4) => self.jsr_c4(bus, ctx), (JSR, 5) => self.jsr_c5(bus, ctx),
            (RTS, 1) => self.rts_c1(bus, ctx), (RTS, 2) => self.rts_c2(bus, ctx), (RTS, 3) => self.rts_c3(bus, ctx), (RTS, 4) => self.rts_c4(bus, ctx), (RTS, 5) => self.rts_c5(bus, ctx),
            (RTI, 1) => self.rti_c1(bus, ctx), (RTI, 2) => self.rti_c2(bus, ctx), (RTI, 3) => self.rti_c3(bus, ctx), (RTI, 4) => self.rti_c4(bus, ctx), (RTI, 5) => self.rti_c5(bus, ctx),

            // --- Stack (Batch 6) ---
            (PHA, 1) => self.pha_c1(bus, ctx), (PHA, 2) => self.pha_c2(bus, ctx), (PHP, 1) => self.php_c1(bus, ctx), (PHP, 2) => self.php_c2(bus, ctx),
            (PLA, 1) => self.pla_c1(bus, ctx), (PLA, 2) => self.pla_c2(bus, ctx), (PLA, 3) => self.pla_c3(bus, ctx), (PLP, 1) => self.plp_c1(bus, ctx), (PLP, 2) => self.plp_c2(bus, ctx), (PLP, 3) => self.plp_c3(bus, ctx),

            // --- Branches ---
            (BNE, 1) => self.branch_c1(bus, ctx, (self.p & 0x02) == 0), (BEQ, 1) => self.branch_c1(bus, ctx, (self.p & 0x02) != 0),
            (BCC, 1) => self.branch_c1(bus, ctx, (self.p & 0x01) == 0), (BCS, 1) => self.branch_c1(bus, ctx, (self.p & 0x01) != 0),
            (BPL, 1) => self.branch_c1(bus, ctx, (self.p & 0x80) == 0), (BMI, 1) => self.branch_c1(bus, ctx, (self.p & 0x80) != 0),
            (BVC, 1) => self.branch_c1(bus, ctx, (self.p & 0x40) == 0), (BVS, 1) => self.branch_c1(bus, ctx, (self.p & 0x40) != 0),
            (_, 2) => self.branch_c2(bus, ctx), (_, 3) => self.branch_c3(bus, ctx),

            // --- Flags ---
            (CLC, 1) => self.clc_c1(bus, ctx), (SEC, 1) => self.sec_c1(bus, ctx), (CLI, 1) => self.cli_c1(bus, ctx), (SEI, 1) => self.sei_c1(bus, ctx), (CLV, 1) => self.clv_c1(bus, ctx), (CLD, 1) => self.cld_c1(bus, ctx), (SED, 1) => self.sed_c1(bus, ctx),

            (NOP, 1) => self.nop_c1(bus, ctx),
            _ => self.unimplemented(bus, ctx),
        }
    }

    /* FIXME! Remove this
    fn fetch_opcode<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if let Some(int) = self.pending_interrupt { match int { Interrupt::Reset => self.opcode = OP_RESET, Interrupt::Nmi => self.opcode = OP_NMI, Interrupt::Irq => self.opcode = OP_IRQ, } self.pending_interrupt = None; bus.read(self.pc, ctx); self.cycle = 1; return; }
        if bus.nmi_pending() { self.opcode = OP_NMI; bus.read(self.pc, ctx); self.cycle = 1; return; }
        if bus.irq_pending() && (self.p & 0x04) == 0 {
            self.opcode = OP_IRQ; bus.read(self.pc, ctx); self.cycle = 1; return;
        }
        self.opcode = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 1;
    } */

    // Addressing
    #[inline(always)] fn ld_zp_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.effective_addr = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn ld_zpx_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.effective_addr, ctx); self.effective_addr = (self.effective_addr + self.x as u16) & 0xFF; self.cycle = 3; }
    #[inline(always)] fn ld_zpy_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.effective_addr, ctx); self.effective_addr = (self.effective_addr + self.y as u16) & 0xFF; self.cycle = 3; }
    #[inline(always)] fn ld_abs_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.effective_addr = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn ld_abs_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.effective_addr |= hi << 8; self.pc = self.pc.wrapping_add(1); self.cycle = 3; }
    #[inline(always)] fn ld_absx_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let lo = self.effective_addr; self.effective_addr = (hi << 8).wrapping_add(lo).wrapping_add(self.x as u16); self.page_crossed = (lo + self.x as u16) > 0xFF; self.cycle = 3; }
    #[inline(always)] fn ld_absy_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let lo = self.effective_addr; self.effective_addr = (hi << 8).wrapping_add(lo).wrapping_add(self.y as u16); self.page_crossed = (lo + self.y as u16) > 0xFF; self.cycle = 3; }
    #[inline(always)] fn ld_absx_write_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let lo = self.effective_addr; bus.read((lo.wrapping_add(self.x as u16) & 0xFF) | (hi << 8), ctx); self.effective_addr = lo.wrapping_add(self.x as u16).wrapping_add(hi << 8); self.cycle = 3; }
    #[inline(always)] fn ld_absy_write_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); let lo = self.effective_addr; bus.read((lo.wrapping_add(self.y as u16) & 0xFF) | (hi << 8), ctx); self.effective_addr = lo.wrapping_add(self.y as u16).wrapping_add(hi << 8); self.cycle = 3; }
    #[inline(always)] fn ld_indx_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.effective_addr, ctx); self.temp_addr = (self.effective_addr + self.x as u16) & 0xFF; self.cycle = 3; }
    #[inline(always)] fn ld_indx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.data_bus = bus.read(self.temp_addr, ctx); self.cycle = 4; }
    #[inline(always)] fn ld_indx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read((self.temp_addr + 1) & 0xFF, ctx) as u16; self.effective_addr = self.data_bus as u16 | (hi << 8); self.cycle = 5; }
    #[inline(always)] fn ld_indy_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.temp_addr = bus.read(self.effective_addr, ctx) as u16; self.cycle = 3; }
    #[inline(always)] fn ld_indy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read((self.effective_addr + 1) & 0xFF, ctx) as u16; let lo = self.temp_addr; self.effective_addr = (hi << 8).wrapping_add(lo).wrapping_add(self.y as u16); self.page_crossed = (lo + self.y as u16) > 0xFF; self.cycle = 4; }
    #[inline(always)] fn ld_indy_write_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read((self.effective_addr + 1) & 0xFF, ctx) as u16; let lo = self.temp_addr; bus.read((lo.wrapping_add(self.y as u16) & 0xFF) | (hi << 8), ctx); self.effective_addr = lo.wrapping_add(self.y as u16).wrapping_add(hi << 8); self.cycle = 4; }

    // RMW Common
    #[inline(always)] fn rmw_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.data_bus = bus.read(self.effective_addr, ctx); self.cycle = 3; }
    #[inline(always)] fn rmw_zp_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.data_bus, ctx); self.cycle = 4; }
    #[inline(always)] fn rmw_zp_c3_read<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.data_bus = bus.read(self.effective_addr, ctx); self.cycle = 4; }
    #[inline(always)] fn rmw_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.data_bus, ctx); self.cycle = 5; }
    #[inline(always)] fn rmw_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.data_bus = bus.read(self.effective_addr, ctx); self.cycle = 4; }
    #[inline(always)] fn rmw_abs_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.data_bus, ctx); self.cycle = 5; }
    #[inline(always)] fn rmw_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.data_bus, ctx); self.cycle = 6; }

    // LDA/X/Y
    #[inline(always)] fn lda_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn lda_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn lda_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn lda_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn lda_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn lda_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn lda_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn lda_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn lda_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ldx_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.x = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn ldx_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.x = bus.read(self.effective_addr, ctx); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn ldx_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.x = bus.read(self.effective_addr, ctx); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn ldx_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.x = bus.read(self.effective_addr, ctx); self.set_nz(self.x); self.cycle = 0; } }
    #[inline(always)] fn ldx_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.x = bus.read(self.effective_addr, ctx); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn ldy_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.y = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.y); self.cycle = 0; }
    #[inline(always)] fn ldy_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.y = bus.read(self.effective_addr, ctx); self.set_nz(self.y); self.cycle = 0; }
    #[inline(always)] fn ldy_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.y = bus.read(self.effective_addr, ctx); self.set_nz(self.y); self.cycle = 0; }
    #[inline(always)] fn ldy_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.y = bus.read(self.effective_addr, ctx); self.set_nz(self.y); self.cycle = 0; } }
    #[inline(always)] fn ldy_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.y = bus.read(self.effective_addr, ctx); self.set_nz(self.y); self.cycle = 0; }

    // STA/X/Y
    #[inline(always)] fn sta_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.a, ctx); self.cycle = 0; }
    #[inline(always)] fn sta_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.a, ctx); self.cycle = 0; }
    #[inline(always)] fn stx_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.x, ctx); self.cycle = 0; }
    #[inline(always)] fn stx_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.x, ctx); self.cycle = 0; }
    #[inline(always)] fn sty_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.y, ctx); self.cycle = 0; }
    #[inline(always)] fn sty_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(self.effective_addr, self.y, ctx); self.cycle = 0; }

    // Transfers
    #[inline(always)] fn tax_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.x = self.a; self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn txa_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.a = self.x; self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn tay_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.y = self.a; self.set_nz(self.y); self.cycle = 0; }
    #[inline(always)] fn tya_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.a = self.y; self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn tsx_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.x = self.s; self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn txs_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.s = self.x; self.cycle = 0; }

    // ALU
    #[inline(always)] fn adc_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn adc_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn adc_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn adc_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; } }
    #[inline(always)] fn adc_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn adc_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; } }
    #[inline(always)] fn adc_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn adc_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; } }
    #[inline(always)] fn adc_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.adc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; } }
    #[inline(always)] fn sbc_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; } }
    #[inline(always)] fn sbc_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn sbc_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; } }
    #[inline(always)] fn sbc_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.sbc(v); self.cycle = 0; }
    #[inline(always)] fn and_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn and_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn and_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn and_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn and_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn and_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn and_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn and_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn and_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a &= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn ora_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn ora_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn ora_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn ora_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a |= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn eor_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn eor_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn eor_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; } }
    #[inline(always)] fn eor_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a ^= bus.read(self.effective_addr, ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn cmp_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cmp_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cmp_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cmp_absx_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; } }
    #[inline(always)] fn cmp_absx_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cmp_absy_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 4; } else { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; } }
    #[inline(always)] fn cmp_absy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cmp_indy_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { if self.page_crossed { bus.read(self.effective_addr.wrapping_sub(0x100), ctx); self.cycle = 5; } else { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; } }
    #[inline(always)] fn cmp_indy_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.a, v); self.cycle = 0; }
    #[inline(always)] fn cpx_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.cmp(self.x, v); self.cycle = 0; }
    #[inline(always)] fn cpx_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.x, v); self.cycle = 0; }
    #[inline(always)] fn cpx_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.x, v); self.cycle = 0; }
    #[inline(always)] fn cpy_imm_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.cmp(self.y, v); self.cycle = 0; }
    #[inline(always)] fn cpy_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.y, v); self.cycle = 0; }
    #[inline(always)] fn cpy_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.cmp(self.y, v); self.cycle = 0; }
    #[inline(always)] fn bit_zp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.bit(v); self.cycle = 0; }
    #[inline(always)] fn bit_abs_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let v = bus.read(self.effective_addr, ctx); self.bit(v); self.cycle = 0; }

    // Shift/Rotate
    #[inline(always)] fn asl_acc_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); let (r, c) = self.a.overflowing_shl(1); self.p = (self.p & !0x83) | (if c || (self.a & 0x80 != 0) { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); self.a = r; self.cycle = 0; }
    #[inline(always)] fn asl_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let (r, c) = self.data_bus.overflowing_shl(1); self.p = (self.p & !0x83) | (if c || (self.data_bus & 0x80 != 0) { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn asl_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let (r, c) = self.data_bus.overflowing_shl(1); self.p = (self.p & !0x83) | (if c || (self.data_bus & 0x80 != 0) { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn asl_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let (r, c) = self.data_bus.overflowing_shl(1); self.p = (self.p & !0x83) | (if c || (self.data_bus & 0x80 != 0) { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn asl_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let (r, c) = self.data_bus.overflowing_shl(1); self.p = (self.p & !0x83) | (if c || (self.data_bus & 0x80 != 0) { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn lsr_acc_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); let c = self.a & 0x01 != 0; let r = self.a >> 1; self.p = (self.p & !0x83) | (if c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }); self.a = r; self.cycle = 0; }
    #[inline(always)] fn lsr_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c = self.data_bus & 0x01 != 0; let r = self.data_bus >> 1; self.p = (self.p & !0x83) | (if c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn lsr_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c = self.data_bus & 0x01 != 0; let r = self.data_bus >> 1; self.p = (self.p & !0x83) | (if c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn lsr_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c = self.data_bus & 0x01 != 0; let r = self.data_bus >> 1; self.p = (self.p & !0x83) | (if c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn lsr_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c = self.data_bus & 0x01 != 0; let r = self.data_bus >> 1; self.p = (self.p & !0x83) | (if c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn rol_acc_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); let c_in = (self.p & 0x01) as u8; let c_out = self.a & 0x80 != 0; let r = (self.a << 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); self.a = r; self.cycle = 0; }
    #[inline(always)] fn rol_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) as u8; let c_out = self.data_bus & 0x80 != 0; let r = (self.data_bus << 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn rol_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) as u8; let c_out = self.data_bus & 0x80 != 0; let r = (self.data_bus << 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn rol_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) as u8; let c_out = self.data_bus & 0x80 != 0; let r = (self.data_bus << 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn rol_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) as u8; let c_out = self.data_bus & 0x80 != 0; let r = (self.data_bus << 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn ror_acc_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); let c_in = (self.p & 0x01) << 7; let c_out = self.a & 0x01 != 0; let r = (self.a >> 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); self.a = r; self.cycle = 0; }
    #[inline(always)] fn ror_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) << 7; let c_out = self.data_bus & 0x01 != 0; let r = (self.data_bus >> 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn ror_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) << 7; let c_out = self.data_bus & 0x01 != 0; let r = (self.data_bus >> 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn ror_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) << 7; let c_out = self.data_bus & 0x01 != 0; let r = (self.data_bus >> 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn ror_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let c_in = (self.p & 0x01) << 7; let c_out = self.data_bus & 0x01 != 0; let r = (self.data_bus >> 1) | c_in; self.p = (self.p & !0x83) | (if c_out { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }

    // INC/DEC
    #[inline(always)] fn inc_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_add(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn inc_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_add(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn inc_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_add(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn inc_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_add(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn dec_zp_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_sub(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn dec_zp_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_sub(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn dec_abs_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_sub(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn dec_abs_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let r = self.data_bus.wrapping_sub(1); self.set_nz(r); bus.write(self.effective_addr, r, ctx); self.cycle = 0; }
    #[inline(always)] fn inx_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.x = self.x.wrapping_add(1); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn iny_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.y = self.y.wrapping_add(1); self.set_nz(self.y); self.cycle = 0; }
    #[inline(always)] fn dex_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.x = self.x.wrapping_sub(1); self.set_nz(self.x); self.cycle = 0; }
    #[inline(always)] fn dey_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.y = self.y.wrapping_sub(1); self.set_nz(self.y); self.cycle = 0; }

    // Jumps/Calls
    #[inline(always)] fn jmp_abs_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.effective_addr = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn jmp_abs_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.effective_addr | (hi << 8); self.cycle = 0; }
    #[inline(always)] fn jmp_ind_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.temp_addr = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn jmp_ind_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.temp_addr |= hi << 8; self.cycle = 3; }
    #[inline(always)] fn jmp_ind_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.effective_addr = bus.read(self.temp_addr, ctx) as u16; self.cycle = 4; }
    #[inline(always)] fn jmp_ind_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi_addr = (self.temp_addr & 0xFF00) | (self.temp_addr.wrapping_add(1) & 0x00FF); let hi = bus.read(hi_addr, ctx) as u16; self.pc = self.effective_addr | (hi << 8); self.cycle = 0; }
    #[inline(always)] fn jsr_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.temp_addr = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn jsr_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.cycle = 3; }
    #[inline(always)] fn jsr_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 4; }
    #[inline(always)] fn jsr_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 5; }
    #[inline(always)] fn jsr_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(self.pc, ctx) as u16; self.pc = self.temp_addr | (hi << 8); self.cycle = 0; }
    #[inline(always)] fn rts_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn rts_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.s = self.s.wrapping_add(1); self.cycle = 3; }
    #[inline(always)] fn rts_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.pc = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.s = self.s.wrapping_add(1); self.cycle = 4; }
    #[inline(always)] fn rts_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.pc |= hi << 8; self.cycle = 5; }
    #[inline(always)] fn rts_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.cycle = 0; }
    #[inline(always)] fn rti_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn rti_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.s = self.s.wrapping_add(1); self.cycle = 3; }
    #[inline(always)] fn rti_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.p = (bus.read(0x0100 | (self.s as u16), ctx) & !0x10) | 0x20; self.s = self.s.wrapping_add(1); self.cycle = 4; }
    #[inline(always)] fn rti_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.pc = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.s = self.s.wrapping_add(1); self.cycle = 5; }
    #[inline(always)] fn rti_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(0x0100 | (self.s as u16), ctx) as u16; self.pc |= hi << 8; self.cycle = 0; }

    // Stack
    #[inline(always)] fn pha_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn pha_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), self.a, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 0; }
    #[inline(always)] fn php_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn php_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 0; }
    #[inline(always)] fn pla_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn pla_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.s = self.s.wrapping_add(1); self.cycle = 3; }
    #[inline(always)] fn pla_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.a = bus.read(0x0100 | (self.s as u16), ctx); self.set_nz(self.a); self.cycle = 0; }
    #[inline(always)] fn plp_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn plp_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.s = self.s.wrapping_add(1); self.cycle = 3; }
    #[inline(always)] fn plp_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.p = (bus.read(0x0100 | (self.s as u16), ctx) & !0x10) | 0x20; self.cycle = 0; }

    // Misc
    #[inline(always)] fn clc_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p &= !0x01; self.cycle = 0; }
    #[inline(always)] fn sec_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p |= 0x01; self.cycle = 0; }
    #[inline(always)] fn cli_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p &= !0x04; self.cycle = 0; }
    #[inline(always)] fn sei_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p |= 0x04; self.cycle = 0; }
    #[inline(always)] fn clv_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p &= !0x40; self.cycle = 0; }
    #[inline(always)] fn cld_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p &= !0x08; self.cycle = 0; }
    #[inline(always)] fn sed_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.p |= 0x08; self.cycle = 0; }
    #[inline(always)] fn nop_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 0; }

    // Helpers
    fn fetch_opcode<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) {
        if let Some(int) = self.pending_interrupt { match int { Interrupt::Reset => self.opcode = OP_RESET, Interrupt::Nmi => self.opcode = OP_NMI, Interrupt::Irq => self.opcode = OP_IRQ, } self.pending_interrupt = None; bus.read(self.pc, ctx); self.cycle = 1; return; }
        if bus.nmi_pending() { self.opcode = OP_NMI; bus.read(self.pc, ctx); self.cycle = 1; return; }
        if bus.irq_pending() && (self.p & 0x04) == 0 {
            self.opcode = OP_IRQ; bus.read(self.pc, ctx); self.cycle = 1; return;
        }
        self.opcode = bus.read(self.pc, ctx) as u16; self.pc = self.pc.wrapping_add(1); self.cycle = 1;
    }
    #[inline(always)] fn branch_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, taken: bool) { let offset = bus.read(self.pc, ctx) as i8; self.pc = self.pc.wrapping_add(1); if taken { self.temp_addr = self.pc.wrapping_add(offset as u16); self.page_crossed = (self.pc & 0xFF00) != (self.temp_addr & 0xFF00); self.cycle = 2; } else { self.cycle = 0; } }
    #[inline(always)] fn branch_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.pc = self.temp_addr; if self.page_crossed { self.cycle = 3; } else { self.cycle = 0; } }
    #[inline(always)] fn branch_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read((self.pc & 0x00FF) | (self.temp_addr & 0xFF00), ctx); self.cycle = 0; }
    #[inline(always)] fn reset_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn reset_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 3; }
    #[inline(always)] fn reset_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.cycle = 4; }
    #[inline(always)] fn reset_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(0x0100 | (self.s as u16), ctx); self.cycle = 5; }
    #[inline(always)] fn reset_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { self.pc = bus.read(VECTOR_RESET, ctx) as u16; self.cycle = 6; }
    #[inline(always)] fn reset_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let hi = bus.read(VECTOR_RESET + 1, ctx) as u16; self.pc |= hi << 8; self.cycle = 0; }
    #[inline(always)] fn int_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.cycle = 2; }
    #[inline(always)] fn int_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 3; }
    #[inline(always)] fn int_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 4; }
    #[inline(always)] fn int_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { let b = if self.opcode == BRK { 0x10 } else { 0 }; bus.write(0x0100 | (self.s as u16), self.p | 0x20 | b, ctx); self.s = self.s.wrapping_sub(1); self.p |= 0x04; self.cycle = 5; }
    #[inline(always)] fn int_c5<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, v: u16) { self.pc = bus.read(v, ctx) as u16; self.cycle = 6; }
    #[inline(always)] fn int_c6<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, v: u16) { let hi = bus.read(v + 1, ctx) as u16; self.pc |= hi << 8; self.cycle = 0; }
    #[inline(always)] fn brk_c1<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.read(self.pc, ctx); self.pc = self.pc.wrapping_add(1); self.cycle = 2; }
    #[inline(always)] fn brk_c2<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc >> 8) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 3; }
    #[inline(always)] fn brk_c3<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), (self.pc & 0xFF) as u8, ctx); self.s = self.s.wrapping_sub(1); self.cycle = 4; }
    #[inline(always)] fn brk_c4<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx) { bus.write(0x0100 | (self.s as u16), self.p | 0x30, ctx); self.s = self.s.wrapping_sub(1); self.p |= 0x04; self.cycle = 5; }
    fn set_nz(&mut self, v: u8) { self.p = (self.p & !0x82) | (if v == 0 { 0x02 } else { 0 }) | (if v & 0x80 != 0 { 0x80 } else { 0 }); }
    fn adc(&mut self, v: u8) {
        if self.p & 0x08 != 0 {
            let a = self.a;
            let b = v;
            let c = self.p & 0x01;
            let mut low = (a & 0x0F) + (b & 0x0F) + c;
            if low > 9 { low += 6; }
            let mut high = (a >> 4) + (b >> 4) + (if low > 15 { 1 } else { 0 });
            let al = a as u16; let bl = b as u16; let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 { self.p |= 0x40; }
            if (res_bin & 0x80) != 0 { self.p |= 0x80; }
            if (res_bin as u8) == 0 { self.p |= 0x02; }
            if high > 9 { high += 6; }
            if high > 15 { self.p |= 0x01; }
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
        } else {
            let a = self.a as u16; let b = v as u16; let c = (self.p & 0x01) as u16;
            let res = a + b + c;
            self.p &= !0xC3;
            if res > 0xFF { self.p |= 0x01; }
            let r = res as u8;
            if r == 0 { self.p |= 0x02; }
            if (r & 0x80) != 0 { self.p |= 0x80; }
            if ((a ^ res) & (b ^ res) & 0x80) != 0 { self.p |= 0x40; }
            self.a = r;
        }
    }
    fn sbc(&mut self, v: u8) {
        if self.p & 0x08 != 0 {
            let a = self.a;
            let b = v;
            let c = self.p & 0x01;
            let mut low = (a & 0x0F) as i32 - (b & 0x0F) as i32 - (1 - c as i32);
            let mut high = (a >> 4) as i32 - (b >> 4) as i32 - (if low < 0 { 1 } else { 0 });
            let al = a as u16; let bl = (!b) as u16; let cl = c as u16;
            let res_bin = al + bl + cl;
            self.p &= !0xC3;
            if res_bin > 0xFF { self.p |= 0x01; }
            if (res_bin as u8) == 0 { self.p |= 0x02; }
            if (res_bin & 0x80) != 0 { self.p |= 0x80; }
            if ((al ^ res_bin) & (bl ^ res_bin) & 0x80) != 0 { self.p |= 0x40; }
            if low < 0 { low -= 6; }
            if high < 0 { high -= 6; }
            self.a = ((low & 0x0F) | ((high & 0x0F) << 4)) as u8;
        } else {
            self.adc(!v);
        }
    }
    fn cmp(&mut self, reg: u8, v: u8) { let (r, c) = reg.overflowing_sub(v); self.p = (self.p & !0x83) | (if !c { 0x01 } else { 0 }) | (if r == 0 { 0x02 } else { 0 }) | (if r & 0x80 != 0 { 0x80 } else { 0 }); }
    fn bit(&mut self, v: u8) { self.p = (self.p & !0xC2) | (if self.a & v == 0 { 0x02 } else { 0 }) | (v & 0xC0); }
    fn unimplemented<B: Bus>(&mut self, _bus: &mut B, _ctx: &mut B::Ctx) { self.cycle = 0; }
}
