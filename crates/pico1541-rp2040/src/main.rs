#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use rp2040_hal::{
    clocks::init_clocks_and_plls,
    multicore::{Multicore, Stack},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};
use pico1541_emu::{
    activetrack::ActiveTrack,
    Emulator,
    Command, HardwareBridge,
};
use heapless::spsc::Queue;

/// Shared memory for the current track.
static ACTIVE_TRACK: ActiveTrack = ActiveTrack::new();

/// SPSC Queue for sending commands from Emulator (Core 0) to Manager (Core 1).
static mut COMMAND_QUEUE: Queue<Command, 16> = Queue::new();

// Allocate a stack for Core 1 (4KB)
static CORE1_STACK: Stack<4096> = Stack::new();

/// This is the entry point for Core 1 (The Manager)
fn core1_task() {
    loop {
        cortex_m::asm::wfi();
    }
}

/// This is the main entry point for Core 0 (The Emulator)
#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let mut sio = Sio::new(pac.SIO);

    // Set up clocks
    let _clocks = init_clocks_and_plls(
        12_000_000u32,
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

    // Split the command queue safely
    let (cmd_tx, _cmd_rx) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };

    // Spawn the manager task on Core 1
    let _ = core1.spawn(CORE1_STACK.take().unwrap(), core1_task);

    // Create the hardware bridge using the static track buffer and the queue producer
    let bridge = HardwareBridge::new(&ACTIVE_TRACK, cmd_tx);

    // Initialize a dummy ROM byte array.
    // In a production build, this would be the actual 1541 ROM.
    let rom = [0u8; 16384];
    
    // Initialize the drive emulator
    let mut drive = Emulator::new(rom, bridge);
    drive.reset();

    loop {
        // Step the emulation by one instruction.
        // In the future, we will use a hardware timer here to ensure 
        // 1MHz execution speed.
        drive.step();
    }
}
