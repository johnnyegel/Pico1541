use crate::systembus::SystemBus;
use pico1541_common::interface::HostInterface;

/// Manages High-Level Emulation (HLE) interventions.
///
/// This layer is responsible for patching the emulator's state (RAM, etc.)
/// to perform operations that would normally be done by the DOS ROM, 
/// but are intercepted for performance or functionality (e.g., hijacking "CD").
pub struct HleManager;

impl HleManager {
    /// Intercepts and discards the current DOS command buffer, 
    /// silencing any "SYNTAX ERROR" or other failure status that would
    /// normally result from a partially received or hijacked command.
    pub fn intercept_command<H: HostInterface>(bus: &mut SystemBus<H>) {
        // 1. Clear the command waiting flag (found at $0255 in 1541-II ROM)
        // This stops the ROM from trying to parse the buffer.
        bus.ram[0x0255] = 0;

        // 2. Clear the start of the command buffer
        bus.ram[0x0200] = 0;

        // 3. Reset the internal error state to "00, OK" (erword at $026C)
        bus.ram[0x026C] = 0;

        // 4. Force the error message buffer to "00, OK"
        let ok_msg = b"00, OK, 00, 00";
        for (i, &b) in ok_msg.iter().enumerate() {
            if 0x02D5 + i < bus.ram.len() {
                bus.ram[0x02D5 + i] = b;
            }
        }
    }
}
