#[derive(Clone, Copy)]
pub struct Gpio {
    pin_mask: u32,
}

impl Gpio {
    /// Create a new masked GPIO controller
    #[inline(always)]
    pub const fn new(pin_mask: u32) -> Self {
        Self { pin_mask }
    }

    /// Read the masked pin states
    #[inline(always)]
    pub fn read(&self) -> u32 {
        embassy_rp::pac::SIO.gpio_in(0).read() & self.pin_mask
    }

    /// Write states to the pins defined by the mask.
    /// Only pins in the mask will be affected.
    #[inline(always)]
    pub fn write(&self, pin_states: u32) {
        Self::write_masked(pin_states, self.pin_mask);
    }

    /// Toggle specific pins, limited by the mask
    #[inline(always)]
    pub fn toggle(&self, pins: u32) {
        Self::toggle_masked(pins, self.pin_mask);
    }

    /// Toggle all pins in the mask
    #[inline(always)]
    pub fn toggle_all(&self) {
        Self::toggle_masked(self.pin_mask, self.pin_mask);
    }

    /// Read raw states of the specified mask without requiring an instance
    #[inline(always)]
    pub fn read_masked(pin_mask: u32) -> u32 {
        embassy_rp::pac::SIO.gpio_in(0).read() & pin_mask
    }

    /// Read all raw states without any masking
    #[inline(always)]
    pub fn read_all() -> u32 {
        embassy_rp::pac::SIO.gpio_in(0).read()
    }

    /// Write raw states to the specified mask without requiring an instance
    #[inline(always)]
    pub fn write_masked(pin_states: u32, pin_mask: u32) {
        let set_mask = pin_states & pin_mask;
        let clr_mask = !pin_states & pin_mask;

        embassy_rp::pac::SIO
            .gpio_out(0)
            .value_clr()
            .write_value(clr_mask);
        embassy_rp::pac::SIO
            .gpio_out(0)
            .value_set()
            .write_value(set_mask);
    }

    /// Toggle raw pins based on the provided mask without requiring an instance
    #[inline(always)]
    pub fn toggle_masked(pins: u32, pin_mask: u32) {
        let toggle_mask = pins & pin_mask;
        embassy_rp::pac::SIO
            .gpio_out(0)
            .value_xor()
            .write_value(toggle_mask);
    }
}
