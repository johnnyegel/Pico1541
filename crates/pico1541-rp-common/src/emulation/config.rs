#![allow(dead_code)]

pub const GPIO_DEBUG_BIT_0: usize = 0;
pub const GPIO_DEBUG_BIT_1: usize = 1;

pub const GPIO_IEC_BIT_DATA_RX: usize = 19;
pub const GPIO_IEC_BIT_DATA_TX: usize = 20;
pub const GPIO_IEC_BIT_CLK_RX: usize = 21;
pub const GPIO_IEC_BIT_CLK_TX: usize = 22;
pub const GPIO_IEC_BIT_ATN_RX: usize = 26;
pub const GPIO_IEC_BIT_RESET: usize = 27;

pub type PinIecDataTx = embassy_rp::peripherals::PIN_20;
pub type PinIecClkTx = embassy_rp::peripherals::PIN_22;
pub type PinIecAtnRx = embassy_rp::peripherals::PIN_26;

pub const GPIO_SYNC_SHADOW_BIT: usize = 24;

pub const GPIO_IEC_MASK_DATA_RX: u32 = 1 << GPIO_IEC_BIT_DATA_RX;
pub const GPIO_IEC_MASK_CLK_RX: u32 = 1 << GPIO_IEC_BIT_CLK_RX;
pub const GPIO_IEC_MASK_ATN_RX: u32 = 1 << GPIO_IEC_BIT_ATN_RX;
pub const GPIO_IEC_MASK_RESET: u32 = 1 << GPIO_IEC_BIT_RESET;

pub const GPIO_LED_BIT_GREEN: usize = 10; // PIN 1
pub const GPIO_LED_BIT_RED: usize = 3; // PIN 2

pub const GPIO_LED_MASK_GREEN: u32 = 1 << GPIO_LED_BIT_GREEN;
pub const GPIO_LED_MASK_RED: u32 = 1 << GPIO_LED_BIT_RED;
