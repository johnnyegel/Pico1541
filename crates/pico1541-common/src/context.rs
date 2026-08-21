pub trait ViaHardware {
    // VIA1 constants for IEC bus
    const VIA1_IEC_BIT_DATA_RX: usize = 0; // PB0
    const VIA1_IEC_BIT_DATA_TX: usize = 1; // PB1
    const VIA1_IEC_BIT_CLK_RX: usize = 2;  // PB2
    const VIA1_IEC_BIT_CLK_TX: usize = 3;  // PB3
    const VIA1_IEC_BIT_ATN_ACK: usize = 4; // PB4
    const VIA1_IEC_BIT_ATN_RX: usize = 7;  // PB7

    const VIA1_IEC_MASK_DATA_RX: u8 = 1 << Self::VIA1_IEC_BIT_DATA_RX; // PB0
    const VIA1_IEC_MASK_DATA_TX: u8 = 1 << Self::VIA1_IEC_BIT_DATA_TX; // PB1
    const VIA1_IEC_MASK_CLK_RX: u8 = 1 << Self::VIA1_IEC_BIT_CLK_RX;  // PB2
    const VIA1_IEC_MASK_CLK_TX: u8 = 1 << Self::VIA1_IEC_BIT_CLK_TX;  // PB3
    const VIA1_IEC_MASK_ATN_ACK: u8 = 1 << Self::VIA1_IEC_BIT_ATN_ACK; // PB4
    const VIA1_IEC_MASK_ATN_RX: u8 = 1 << Self::VIA1_IEC_BIT_ATN_RX;  // PB7
    
    // VIA2 constants for motor and LED control
    const VIA2_BIT_MOTOR: usize = 2; // PB2
    const VIA2_BIT_LED: usize = 3;   // PB3

    const VIA2_MASK_MOTOR:  u8 = 1 << Self::VIA2_BIT_MOTOR; // PB2
    const VIA2_MASK_LED:    u8 = 1 << Self::VIA2_BIT_LED;   // PB3
    const VIA2_MASK_LED_MOTOR: u8 = Self::VIA2_MASK_MOTOR | Self::VIA2_MASK_LED;

    // Hardware interface methods (HAL)

    /// Sets the reset state of the drive, so the hardware can configure LEDs or other indicators accordingly.
    fn set_reset_state(&mut self, _asserted: bool) {}

    /// Reads the current state of the IEC bus lines (DATA, CLK, ATN) as seen by the drive.
    fn read_iec_port(&mut self) -> u8 { 0xFF }

    /// Writes the state of the IEC bus lines (DATA, CLK) as driven by the drive.
    fn write_iec_port(&mut self, _via_port_b: u8) {}

    /// Writes the state of the drive mechanics control lines (MOTOR, LED) as driven by the drive.
    fn write_mechanics_port(&mut self, _via_port_b: u8) {}

    /// Checks if a reset trigger has been consumed by the drive.
    fn consume_trigger_reset(&mut self) -> bool { false }

    /// Checks if the reset line is currently asserted.
    fn is_reset_line_asserted(&self) -> bool { false }
}
