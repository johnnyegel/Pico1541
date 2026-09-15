use embassy_rp::pac::BUSCTRL;

/// Configures the 4 RP2040 performance counters to track AHB collisions on SRAM 0-3.
#[inline(always)]
pub fn configure_sram_contention_counters() {
    // Assign the specific events to the 4 available counters
    BUSCTRL.perfsel(0).write(|w| w.set_perfsel(14.into())); // SRAM0 contested
    BUSCTRL.perfsel(1).write(|w| w.set_perfsel(12.into())); // SRAM1 contested
    BUSCTRL.perfsel(2).write(|w| w.set_perfsel(10.into())); // SRAM2 contested
    BUSCTRL.perfsel(3).write(|w| w.set_perfsel(8.into())); // SRAM3 contested
}

/// Clears all 4 counters to 0, effectively starting the profiling.
#[link_section = ".core1_text"]
#[inline(always)]
pub fn start_profiling() {
    BUSCTRL
        .perfctr(0)
        .write_value(embassy_rp::pac::busctrl::regs::Perfctr(0));
    BUSCTRL
        .perfctr(1)
        .write_value(embassy_rp::pac::busctrl::regs::Perfctr(0));
    BUSCTRL
        .perfctr(2)
        .write_value(embassy_rp::pac::busctrl::regs::Perfctr(0));
    BUSCTRL
        .perfctr(3)
        .write_value(embassy_rp::pac::busctrl::regs::Perfctr(0));
}

/// Reads the counters, and prints a micro-stall report to the RTT log if any collision occurred.
#[link_section = ".core1_text"]
#[inline(never)]
pub fn snapshot_profiling() {
    let collisions_0 = BUSCTRL.perfctr(0).read();
    let collisions_1 = BUSCTRL.perfctr(1).read();
    let collisions_2 = BUSCTRL.perfctr(2).read();
    let collisions_3 = BUSCTRL.perfctr(3).read();

    let total_collisions = collisions_0.0 + collisions_1.0 + collisions_2.0 + collisions_3.0;

    if total_collisions > 0 {
        defmt::warn!(
            "AHB Contention! Total: {} (SRAM0: {}, SRAM1: {}, SRAM2: {}, SRAM3: {})",
            total_collisions,
            collisions_0.0,
            collisions_1.0,
            collisions_2.0,
            collisions_3.0
        );
    }
}
