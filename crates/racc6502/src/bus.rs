/// Trait defining the interface between the CPU and the rest of the system.
///
/// The `Bus` is the primary interface for cycle-accurate interaction with memory and peripherals.
/// In `racc6502`, the bus effectively serves as the system clock.
///
/// `Ctx` is an external context type threaded through every read/write call.  For simple
/// buses it is `()`.  For the 1541 emulator it is the `HostInterface` implementation, which
/// is owned by the caller and never stored inside the bus itself.
pub trait Bus {
    /// The external context passed to every `read` / `write` call.
    /// Use `()` for buses that need no external state.
    type Ctx;

    /// Reads a byte from the given address.
    ///
    /// Called exactly once per CPU cycle during `CPU::tick()` unless `write` is called.
    /// This may be a meaningful read (opcode fetch, data access) or a "dummy" read
    /// used to maintain hardware-accurate timing.
    fn read(&mut self, addr: u16, ctx: &mut Self::Ctx) -> u8;

    /// Writes a byte to the given address.
    ///
    /// Called exactly once per CPU cycle during `CPU::tick()` unless `read` is called.
    fn write(&mut self, addr: u16, value: u8, ctx: &mut Self::Ctx);

    /// Sampled by the CPU at the start of every opcode fetch to determine if an IRQ
    /// sequence should be triggered. Returns `true` if an IRQ is active.
    fn irq_pending(&self) -> bool {
        false
    }

    /// Sampled by the CPU at the start of every opcode fetch to determine if an NMI
    /// sequence should be triggered. Returns `true` if an NMI is active.
    fn nmi_pending(&self) -> bool {
        false
    }

    /// Simulates the hardware `SO` (Set Overflow) pin.
    ///
    /// This pin is sampled by the CPU at the start of **every** clock cycle.
    /// If it returns `true`, the Overflow (V) flag in the status register is set.
    fn so_pending(&self) -> bool {
        false
    }
}

/// Optional trait which can be implemented by a bus for monitoring memory content.
/// This is required for features like breakpoints and watchpoints, but is not necessary for basic CPU operation.
pub trait MemoryMonitor {
    /// Reads a byte from the given address.
    /// This does NOT affect the bus timing!
    fn peek(&self, addr: u16) -> u8;

    /// Writes a byte to the given address.
    /// This does NOT affect the bus timing!
    fn poke(&mut self, addr: u16, value: u8);
}
