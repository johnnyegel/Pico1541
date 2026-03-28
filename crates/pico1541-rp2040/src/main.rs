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
use pico1541_emulator::{
    Emulator,
    interface::{ActiveTrack, DriveEvent, HostInterface, IecBus},
};

/// Local HostInterface implementation for the RP2040.
/// Owns the active track buffer and handles events on Core 0.
/// TODO: forward events to Core 1 via the SIO FIFO for manager processing.
struct HardwareBridge {
    track: ActiveTrack,
    iec_bus: IecBus,
}

impl HardwareBridge {
    fn new() -> Self {
        Self {
            track: ActiveTrack::new(),
            iec_bus: IecBus(0x00),
        }
    }
}

impl HostInterface for HardwareBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        &mut self.track
    }

    fn send_event(&mut self, _event: DriveEvent) {
        // TODO: forward to Core 1 via SIO FIFO
    }

    fn read_iec(&self) -> IecBus {
        self.iec_bus
    }

    fn write_iec(&mut self, pins: IecBus) {
        self.iec_bus = pins;
    }
}

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

    // Spawn the manager task on Core 1
    let _ = core1.spawn(CORE1_STACK.take().unwrap(), core1_task);

    // Create the hardware bridge
    let bridge = HardwareBridge::new();

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
