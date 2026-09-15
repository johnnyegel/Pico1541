#![allow(dead_code)]

pub const GPIO_DEBUG_BIT_0: usize = 0;
pub const GPIO_DEBUG_BIT_1: usize = 1;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_DATA_RX: usize = 16;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_CLK_RX: usize = 17;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_ATN_RX: usize = 18;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_DATA_TX: usize = 19;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_CLK_TX: usize = 20;
#[cfg(feature = "rp2040")]
pub const GPIO_IEC_BIT_RESET: usize = 21;

#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_DATA_RX: usize = 19;
#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_DATA_TX: usize = 20;
#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_CLK_RX: usize = 21;
#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_CLK_TX: usize = 22;
#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_ATN_RX: usize = 26;
#[cfg(feature = "rp235xa")]
pub const GPIO_IEC_BIT_RESET: usize = 27;

#[cfg(feature = "rp2040")]
pub type PinIecDataTx = embassy_rp::peripherals::PIN_19;
#[cfg(feature = "rp2040")]
pub type PinIecClkTx = embassy_rp::peripherals::PIN_20;
#[cfg(feature = "rp2040")]
pub type PinIecAtnRx = embassy_rp::peripherals::PIN_18;

#[cfg(feature = "rp235xa")]
pub type PinIecDataTx = embassy_rp::peripherals::PIN_20;
#[cfg(feature = "rp235xa")]
pub type PinIecClkTx = embassy_rp::peripherals::PIN_22;
#[cfg(feature = "rp235xa")]
pub type PinIecAtnRx = embassy_rp::peripherals::PIN_26;

#[cfg(feature = "rp2040")]
pub const GPIO_SYNC_SHADOW_BIT: usize = 10;
#[cfg(feature = "rp235xa")]
pub const GPIO_SYNC_SHADOW_BIT: usize = 24;

pub const GPIO_IEC_MASK_DATA_RX: u32 = 1 << GPIO_IEC_BIT_DATA_RX;
pub const GPIO_IEC_MASK_CLK_RX: u32 = 1 << GPIO_IEC_BIT_CLK_RX;
pub const GPIO_IEC_MASK_ATN_RX: u32 = 1 << GPIO_IEC_BIT_ATN_RX;
pub const GPIO_IEC_MASK_RESET: u32 = 1 << GPIO_IEC_BIT_RESET;

#[cfg(feature = "rp2040")]
pub const GPIO_LED_BIT_GREEN: usize = 22;
#[cfg(feature = "rp2040")]
pub const GPIO_LED_BIT_RED: usize = 3;

#[cfg(feature = "rp235xa")]
pub const GPIO_LED_BIT_GREEN: usize = 10; // PIN 1
#[cfg(feature = "rp235xa")]
pub const GPIO_LED_BIT_RED: usize = 3; // PIN 2

pub const GPIO_LED_MASK_GREEN: u32 = 1 << GPIO_LED_BIT_GREEN;
pub const GPIO_LED_MASK_RED: u32 = 1 << GPIO_LED_BIT_RED;
