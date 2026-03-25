#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrMicroOp {
    FetchAddrLo,
    FetchAddrHi,
    AddXToZP,
    AddYToZP,
    AddXToAbs,
    AddYToAbs,
    DummyReadPC,
    DummyReadEff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMicroOp {
    // Memory Access (Read)
    ReadEff,
    ReadEffAndLDA,
    ReadEffAndLDX,
    ReadEffAndLDY,
    ReadEffAndADC,
    ReadEffAndSBC,
    ReadEffAndAND,
    ReadEffAndORA,
    ReadEffAndEOR,
    ReadEffAndBIT,
    ReadEffAndCMP,
    ReadEffAndCPX,
    ReadEffAndCPY,

    // Immediate Access (Read PC directly in Operation Phase)
    FetchImmAndLDA,
    FetchImmAndLDX,
    FetchImmAndLDY,
    FetchImmAndADC,
    FetchImmAndSBC,
    FetchImmAndAND,
    FetchImmAndORA,
    FetchImmAndEOR,
    FetchImmAndCMP,
    FetchImmAndCPX,
    FetchImmAndCPY,

    // Memory Access (Write)
    WriteEffA,
    WriteEffX,
    WriteEffY,

    // Stack Operations
    PushA, PushP, PushPCH, PushPCL,
    PullA, PullP, PullPCH, PullPCL,

    // Internal State / Dummy Reads
    DummyReadPC,
    DummyReadEff,

    // Register Transfers & Immediate Flag/ALU
    TAX, TXA, TAY, TYA, TSX, TXS,
    CLC, SEC, CLI, SEI, CLV, CLD, SED,
    INX, DEX, INY, DEY,

    // Jump/Branch
    JumpAbs,
    JumpInd,
    BranchRel,

    Unimplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub addr: &'static [AddrMicroOp],
    pub exec: &'static [ExecMicroOp],
}

pub mod nmos6502;

pub const VECTOR_NMI: u16 = 0xFFFA;
pub const VECTOR_RESET: u16 = 0xFFFC;
pub const VECTOR_IRQ: u16 = 0xFFFE;
