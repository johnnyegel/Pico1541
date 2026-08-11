pub const GPIO_DEBUG_BIT_0: usize = 0;
pub const GPIO_DEBUG_BIT_1: usize = 1;
pub const GPIO_IEC_BIT_DATA_RX: usize = 16; // GPIO 16 (PB0)
pub const GPIO_IEC_BIT_CLK_RX: usize = 17;  // GPIO 17 (PB2)
pub const GPIO_IEC_BIT_ATN_RX: usize = 18;  // GPIO 18 (PB7)
pub const GPIO_IEC_BIT_DATA_TX: usize = 19; // GPIO 19 (PB0)
pub const GPIO_IEC_BIT_CLK_TX: usize = 20;  // GPIO 20 (PB1)
pub const GPIO_IEC_BIT_RESET: usize = 21;   // GPIO 21 (PB6)

pub const GPIO_DEBUG_MASK_0: u32 = 1 << GPIO_DEBUG_BIT_0;
pub const GPIO_DEBUG_MASK_1: u32 = 1 << GPIO_DEBUG_BIT_1;
pub const GPIO_IEC_MASK_DATA_RX: u32 = 1 << GPIO_IEC_BIT_DATA_RX;
pub const GPIO_IEC_MASK_CLK_RX: u32 = 1 << GPIO_IEC_BIT_CLK_RX;
pub const GPIO_IEC_MASK_ATN_RX: u32 = 1 << GPIO_IEC_BIT_ATN_RX;
pub const GPIO_IEC_MASK_DATA_TX: u32 = 1 << GPIO_IEC_BIT_DATA_TX;
pub const GPIO_IEC_MASK_CLK_TX: u32 = 1 << GPIO_IEC_BIT_CLK_TX;
pub const GPIO_IEC_MASK_RESET: u32 = 1 << GPIO_IEC_BIT_RESET;

pub const GPIO_LED_BIT_GREEN: usize = 22;  // GPIO 22 (LED)
pub const GPIO_LED_BIT_RED: usize = 3;   // GPIO 3 (LED)

pub const GPIO_LED_MASK_GREEN: u32 = 1 << GPIO_LED_BIT_GREEN;
pub const GPIO_LED_MASK_RED: u32 = 1 << GPIO_LED_BIT_RED;
