pub trait Bus {
    type Ctx;
    
    /// Reads a byte from memory.
    fn read(&mut self, ctx: &mut Self::Ctx, addr: u16, cycle: u32) -> u8;
    
    /// Extremely fast read strictly for instruction/operand fetching (no IO checks).
    fn get_instruction_ptr(&self, addr: u16) -> *const u8;
    
    /// Writes a byte to memory.
    fn write(&mut self, ctx: &mut Self::Ctx, addr: u16, val: u8, cycle: u32);
    
    /// Optional: Peek memory without triggering side effects.
    #[allow(unused_variables)]
    fn peek(&self, addr: u16) -> u8 { 0 }

    /// Optional: Check if IRQ is pending.
    #[inline(always)]
    fn irq_pending(&self) -> bool { false }
}
