#![no_std]

#[allow(unused_macros)]
#[cfg(not(feature = "logging"))]
macro_rules! emu_info { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(not(feature = "logging"))]
macro_rules! emu_warn { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(not(feature = "logging"))]
macro_rules! emu_debug { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(feature = "logging")]
macro_rules! emu_info { ($($arg:tt)*) => { log::info!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(feature = "logging")]
macro_rules! emu_warn { ($($arg:tt)*) => { log::warn!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(feature = "logging")]
macro_rules! emu_debug { ($($arg:tt)*) => { log::debug!($($arg)*) } }

pub mod drivemechanics;
pub mod hle;
pub mod systembus;
pub mod via6522;

use pico1541_common::interface::HostInterface;
use racc6502::cpu::{CycleState, CPU};
use systembus::SystemBus;

/// The MOS 1541 Drive Emulator.
///
/// `Emulator` holds only the CPU registers and the memory bus (RAM, ROM, VIAs).
/// The `HostInterface` implementation is **not** stored here — callers own it
/// and pass it as `&mut H` to every [`tick`](Self::tick) / [`step`](Self::step) call.
pub struct Emulator<H: HostInterface> {
    pub cpu: CPU,
    pub bus: SystemBus<H>,
}

impl<H: HostInterface> Emulator<H> {
    /// Constructs a new 1541 Emulator with the provided ROM and device ID (8-11).
    pub fn new(rom: [u8; 16384], device_id: u8) -> Self {
        Self {
            cpu: CPU::new(),
            bus: SystemBus::new(rom, device_id),
        }
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self) {
        emu_debug!("Emulator Reset");
        self.cpu.reset();
        self.bus.reset();
    }

    /// Executes exactly one CPU clock cycle, passing `hw` to the bus for all
    /// peripheral interaction during that cycle.
    #[link_section = ".ramfunc"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) {
        self.cpu.tick(&mut self.bus, hw);
    }

    /// Executes the given number of CPU cycles.
    pub fn run_for(&mut self, hw: &mut H, cycles: u32) {
        for _ in 0..cycles {
            self.tick(hw);
        }
    }

    /// Executes a single instruction (ticks until the next opcode fetch).
    pub fn step(&mut self, hw: &mut H) {
        loop {
            self.tick(hw);
            if self.cpu.state == CycleState::FetchOpcode && self.cpu.step_index == 0 {
                break;
            }
        }
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn half_track(&self) -> u8 {
        self.bus.mechanics.half_track()
    }

    /// Direct access to the internal memory bus (e.g. for inspection or peek/poke).
    pub fn bus(&self) -> &SystemBus<H> {
        &self.bus
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H> {
        &mut self.bus
    }
}
