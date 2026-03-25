/// Trait that implements the bus for the 6502 CPU. This is used to read and write memory and peripherals.
/// 
/// The 6502 CPU will call these methods to interact with memory and peripherals. The bus is responsible 
/// for routing these calls to the appropriate devices based on the address.
pub trait Bus {
    /// Reads a byte from the given address.
    fn read(&mut self, addr: u16) -> u8;

    /// Writes a byte to the given address.
    fn write(&mut self, addr: u16, value: u8);

    /// Indicates that an IRQ interrupt is pending. The CPU will check this flag at the end of each instruction and handle it if set.
    fn irq_pending(&self) -> bool { false }

    /// Indicates that an NMI interrupt is pending. The CPU will check this flag at the end of each instruction and handle it if set.
    fn nmi_pending(&self) -> bool { false }

    /// Indicates that the SO (Serial Output) signal is pending. The CPU will check this flag at the end of each instruction and handle it if set.
    fn so_pending(&self) -> bool { false }

    /// Indicates that the RDY (Ready) signal is pending. The CPU will check this flag at the end of each instruction and handle it if set.
    fn rdy_pending(&self) -> bool { false }
}
