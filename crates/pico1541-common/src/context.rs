pub trait CatchUpContext {
    fn catch_up(&mut self, target_cycle: u64);
    fn advance_to(&mut self, target_cycle: u64);
}

pub trait ViaHardware {
    const IEC_DATA_RX: u8 = 0b0000_0001; // PB0
    const IEC_DATA_TX: u8 = 0b0000_0010; // PB1
    const IEC_CLK_RX:  u8 = 0b0000_0100; // PB2
    const IEC_CLK_TX:  u8 = 0b0000_1000; // PB3
    const IEC_ATNA:    u8 = 0b0001_0000; // PB4
    const IEC_ATN_RX:  u8 = 0b1000_0000; // PB7
    
    const VIA2_MOTOR:  u8 = 0b0000_0100; // PB2
    const VIA2_LED:    u8 = 0b0000_1000; // PB3
    const VIA2_LED_MOTOR_MASK: u8 = Self::VIA2_MOTOR | Self::VIA2_LED;

    fn read_iec_port(&mut self) -> u8 { 0xFF }
    fn write_iec_port(&mut self, _via_port_b: u8) {}
    fn write_mechanics_port(&mut self, _via_port_b: u8) {}
    fn consume_trigger_reset(&mut self) -> bool { false }
    fn is_reset_line_asserted(&self) -> bool { false }
}
