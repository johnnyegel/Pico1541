/// Addressing micro-operations that occur during the address-calculation phase of an instruction.
/// 
/// Each variant represents exactly one clock cycle of bus activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrMicroOp {
    /// Fetch the low byte of an address from PC and increment PC.
    FetchAddrLo,
    /// Fetch the high byte of an address from PC and increment PC.
    FetchAddrHi,
    /// Special variant for JMP Abs: fetch high byte and update PC immediately.
    FetchAddrHiAndJump,
    /// Add X to the current effective address (Zero Page). Performs dummy read.
    AddXToZP,
    /// Add Y to the current effective address (Zero Page). Performs dummy read.
    AddYToZP,
    /// Add X to the current effective address (Absolute). May set page_crossed flag.
    AddXToAbs,
    /// Add Y to the current effective address (Absolute). May set page_crossed flag.
    AddYToAbs,
    /// Indirect X: Fetch low byte from ZP address.
    FetchIndXLo,
    /// Indirect X: Fetch high byte from ZP+1 address.
    FetchIndXHi,
    /// Indirect Y: Fetch low byte from ZP address.
    FetchIndYLo,
    /// Indirect Y: Fetch high byte from ZP+1 address.
    FetchIndYHi,
    /// Optional cycle: Correct the high byte of the address if a page was crossed (for reads).
    OptionalFixupX,
    /// Optional cycle: Correct the high byte of the address if a page was crossed (for reads).
    OptionalFixupY,
    /// Optional cycle: Correct the high byte of the address if a page was crossed (for reads).
    OptionalFixupIndY,
    /// Mandatory cycle: Read from the intermediate address and correct high byte (for writes/RMW).
    MandatoryFixupX,
    /// Mandatory cycle: Read from the intermediate address and correct high byte (for writes/RMW).
    MandatoryFixupY,
    /// Mandatory cycle: Read from the intermediate address and correct high byte (for writes/RMW).
    MandatoryFixupIndY,
    /// Perform a dummy read from the current Program Counter.
    DummyReadPC,
    /// Perform a dummy read from the current effective address.
    DummyReadEff,
}

/// Execution micro-operations that occur during the operation phase of an instruction.
/// 
/// Each variant represents exactly one clock cycle of bus activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMicroOp {
    // --- Memory Read Ops ---
    ReadEffAndLDA, ReadEffAndLDX, ReadEffAndLDY,
    ReadEffAndADC, ReadEffAndSBC, ReadEffAndAND,
    ReadEffAndORA, ReadEffAndEOR, ReadEffAndBIT,
    ReadEffAndCMP, ReadEffAndCPX, ReadEffAndCPY,

    // --- Immediate Read Ops ---
    FetchImmAndLDA, FetchImmAndLDX, FetchImmAndLDY,
    FetchImmAndADC, FetchImmAndSBC, FetchImmAndAND,
    FetchImmAndORA, FetchImmAndEOR,
    FetchImmAndCMP, FetchImmAndCPX, FetchImmAndCPY,

    // --- Write Ops ---
    WriteEffA, WriteEffX, WriteEffY,

    // --- Read-Modify-Write Ops ---
    ReadEffToData, WriteDataBack,
    ASLWrite, LSRWrite, ROLWrite, RORWrite, INCWrite, DECWrite,
    AccASL, AccLSR, AccROL, AccROR,

    // --- Stack Ops ---
    PushA, PushP, PushPCH, PushPCL,
    PullA, PullP, PullPCH, PullPCL,
    DummyReadStack,

    // --- Register & Flag Ops ---
    TAX, TXA, TAY, TYA, TSX, TXS,
    CLC, SEC, CLI, SEI, CLV, CLD, SED,
    INX, DEX, INY, DEY,

    // --- Flow Control Ops ---
    JumpIndCycle4, JumpIndCycle5,
    BranchCycle2, BranchCycle3, BranchCycle4,
    JsrCycle2, JsrCycle3, JsrCycle4, JsrCycle5, JsrCycle6,
    RtsCycle2, RtsCycle3, RtsCycle4, RtsCycle5, RtsCycle6,
    RtiCycle2, RtiCycle3, RtiCycle4, RtiCycle5, RtiCycle6,
    BrkCycle2, BrkCycle3, BrkCycle4, BrkCycle5, BrkCycle6, BrkCycle7,

    // --- Misc ---
    DummyReadPC, DummyReadEff, Unimplemented,
}

/// Represents a single 6502 instruction as a sequence of micro-operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// The sequence of cycles used to calculate the effective address.
    pub addr: &'static [AddrMicroOp],
    /// The sequence of cycles used to execute the core instruction logic.
    pub exec: &'static [ExecMicroOp],
}

pub mod nmos6502;

/// Memory address of the Non-Maskable Interrupt vector.
pub const VECTOR_NMI: u16 = 0xFFFA;
/// Memory address of the System Reset vector.
pub const VECTOR_RESET: u16 = 0xFFFC;
/// Memory address of the Interrupt Request / Break vector.
pub const VECTOR_IRQ: u16 = 0xFFFE;
