use cortex_m::peripheral::SYST;

fn setup_syst() {
    let mut syst = unsafe { SYST::steal() };
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(0x00FFFFFF);
    syst.clear_current();
    syst.enable_counter();
}
