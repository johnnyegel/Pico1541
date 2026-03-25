/// Trait defining the interface between the CPU and the rest of the system.
/// 
/// The `Bus` is the primary interface for cycle-accurate interaction with memory and peripherals.
/// In `racc6502`, the bus effectively serves as the system clock.
pub trait Bus {
    /// Reads a byte from the given address.
    /// 
    /// Called exactly once per CPU cycle during `CPU::tick()` unless `write` is called.
    /// This may be a meaningful read (opcode fetch, data access) or a "dummy" read
    /// used to maintain hardware-accurate timing.
    fn read(&mut self, addr: u16) -> u8;

    /// Writes a byte to the given address.
    /// 
    /// Called exactly once per CPU cycle during `CPU::tick()` unless `read` is called.
    fn write(&mut self, addr: u16, value: u8);

    /// Sampled by the CPU at the start of every opcode fetch to determine if an IRQ
    /// sequence should be triggered. Returns `true` if an IRQ is active.
    fn irq_pending(&self) -> bool { false }

    /// Sampled by the CPU at the start of every opcode fetch to determine if an NMI
    /// sequence should be triggered. Returns `true` if an NMI is active.
    fn nmi_pending(&self) -> bool { false }

    /// Simulates the hardware `SO` (Set Overflow) pin.
    /// 
    /// This pin is sampled by the CPU at the start of **every** clock cycle.
    /// If it returns `true`, the Overflow (V) flag in the status register is set.
    fn so_pending(&self) -> bool { false }
}
