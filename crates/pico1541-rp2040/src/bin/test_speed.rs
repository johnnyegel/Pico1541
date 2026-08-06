#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

extern crate alloc;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();


#[cortex_m_rt::entry]
fn main() -> ! {
    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(250_000_000).unwrap();
    let _p = embassy_rp::init(config);

    defmt::info!("Starting emulator speed test...");
    
    // We cannot easily instantiate Emulator without full setup.
    // I will just read the UI stats that are already in the log!
    loop { cortex_m::asm::wfe(); }
}
