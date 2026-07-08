pub trait Bus {
    type Ctx;
    
    /// Reads a byte from memory.
    fn read(&mut self, addr: u16, cycle: u32, ctx: &mut Self::Ctx) -> u8;
    
    /// Writes a byte to memory.
    fn write(&mut self, addr: u16, val: u8, cycle: u32, ctx: &mut Self::Ctx);
    
    /// Optional: Peek memory without triggering side effects.
    #[allow(unused_variables)]
    fn peek(&self, addr: u16) -> u8 { 0 }

    /// Optional: Check if NMI is pending.
    #[inline(always)]
    fn nmi_pending(&self) -> bool { false }

    /// Optional: Check if IRQ is pending.
    #[inline(always)]
    fn irq_pending(&self) -> bool { false }
}
