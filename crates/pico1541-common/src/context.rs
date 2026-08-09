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
    const VIA1_IEC_MASK_ATN_RX: u8 = 1 << Self::VIA1_IEC_BIT_ATN_RX;  // PB7
    
    // VIA2 constants for motor and LED control
    const VIA2_BIT_MOTOR: usize = 2; // PB2
    const VIA2_BIT_LED: usize = 3;   // PB3

    const VIA2_MASK_MOTOR:  u8 = 1 << Self::VIA2_BIT_MOTOR; // PB2
    const VIA2_MASK_LED:    u8 = 1 << Self::VIA2_BIT_LED;   // PB3
    const VIA2_MASK_LED_MOTOR: u8 = Self::VIA2_MASK_MOTOR | Self::VIA2_MASK_LED;

    // Hardware interface methods (HAL)
    fn read_iec_port(&mut self) -> u8 { 0xFF }
    fn write_iec_port(&mut self, _via_port_b: u8) {}
    fn write_mechanics_port(&mut self, _via_port_b: u8) {}
    fn consume_trigger_reset(&mut self) -> bool { false }
    fn is_reset_line_asserted(&self) -> bool { false }
}
