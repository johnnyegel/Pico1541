#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

#[link_section = ".data.ram_func"]
#[inline(never)]
fn run_test() -> (u32, u32) {
    let t1 = embassy_rp::pac::TIMER.timelr().read();
    
    unsafe {
        core::arch::asm!(
            "1:",
            "subs {0}, #1",
            "bne 1b",
            inout(reg) 10_000_000 => _
        );
    }
    
    let t2 = embassy_rp::pac::TIMER.timelr().read();
    
    (t1, t2)
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(250_000_000).unwrap();
    let _p = embassy_rp::init(config);

    defmt::info!("Measuring 10M custom loop iterations...");
    
    let (t1, t2) = run_test();
    
    let diff = t2.wrapping_sub(t1);
    defmt::info!("t1={}, t2={}, diff={} us", t1, t2, diff);
    
    loop {
        cortex_m::asm::wfe();
    }
}
