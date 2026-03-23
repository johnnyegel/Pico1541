#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _; // Halts on panic. In the future, change to a USB logging panicking crate.
use rp2040_hal::{
    clocks::init_clocks_and_plls,
    multicore::{Multicore, Stack},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};

// Allocate a stack for Core 1 (4KB)
static CORE1_STACK: Stack<4096> = Stack::new();

/// This is the entry point for Core 1 (The Manager)
fn core1_task() {
    // 1. Initialize USB Host / SD Card SPI
    // 2. Initialize Display / Buttons
    // 3. Run the pico1541_manager loop to handle generic operations
    
    // Example call: pico1541_manager::run_manager_loop();
    loop {
        cortex_m::asm::wfi(); // Wait for interrupt (sleep to save power for now)
    }
}

/// This is the main entry point for Core 0 (The Emulator)
#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let mut sio = Sio::new(pac.SIO);

    // Set up clocks (133 MHz standard)
    let _clocks = init_clocks_and_plls(
        12_000_000u32, // External crystal frequency
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Initialize Multicore capability
    let mut mc = Multicore::new(&mut pac.PSM, &mut pac.PPB, &mut sio.fifo);
    let cores = mc.cores();
    let core1 = &mut cores[1];

    // Spawn the manager task on Core 1
    let _ = core1.spawn(CORE1_STACK.take().unwrap(), core1_task);

    // Core 0: Initialize PIO for the IEC Bus
    // Core 0: Run the cycle-exact 6502 emulator
    
    // Example call: pico1541_emu::run_emulator_loop();
    loop {
        cortex_m::asm::nop(); // Placeholder for 6502 execution cycle
    }
}