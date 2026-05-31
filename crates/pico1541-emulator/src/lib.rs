#![cfg_attr(not(test), no_std)]

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
use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use systembus::SystemBus;

/// Trait for peeking and poking at memory/registers without side effects.
#[cfg(feature = "debug")]
pub trait MemoryMonitor {
    fn peek(&self, addr: u16) -> u8;
    fn poke(&mut self, addr: u16, value: u8);
}

/// The MOS 1541 Drive Emulator.
///
/// `Emulator` holds only the CPU registers and the memory bus (RAM, ROM, VIAs).
/// The `HostInterface` implementation is **not** stored here — callers own it
/// and pass it as `&mut H` to every [`tick`](Self::tick) / [`step`](Self::step) call.
pub struct Emulator<H: HostInterface> {
    pub cpu: CPU<SystemBus<H>, Nmos6502>,
}

impl<H: HostInterface> Emulator<H> {
    pub fn new(rom: &'static [u8; 16384], device_id: u8) -> Self {
        let bus = SystemBus::new(rom, device_id);
        Self {
            cpu: CPU::new(bus, Nmos6502),
        }
    }

    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.cpu.memory.update_rom(rom);
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self, hw: &mut H) {
        emu_debug!("Emulator Reset");
        self.cpu.memory.reset();
        self.cpu.memory.current_hw = hw;
        self.cpu.reset();
        self.cpu.memory.current_hw = core::ptr::null_mut();
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        self.cpu.memory.current_hw = hw;
        
        let start_cycles = self.cpu.cycles;
        self.cpu.single_step();
        let elapsed = self.cpu.cycles.saturating_sub(start_cycles) as u32;
        
        for _ in 0..elapsed.max(1) {
            self.cpu.memory.tick_cycle(hw);
            if self.cpu.memory.so_pending() {
                self.cpu.registers.status.insert(mos6502::registers::Status::PS_OVERFLOW);
            }
        }
        
        self.cpu.memory.current_hw = core::ptr::null_mut();
        elapsed
    }

    /// Executes the given number of CPU cycles.
    pub fn run_for(&mut self, hw: &mut H, cycles: u32) {
        let start = self.cpu.cycles;
        while self.cpu.cycles.saturating_sub(start) < (cycles as u64) {
            self.tick(hw);
        }
    }

    pub fn step(&mut self, hw: &mut H) {
        self.tick(hw);
    }

    pub fn pc(&self) -> u16 {
        self.cpu.registers.program_counter
    }

    pub fn state(&self) -> (u8, u8, u8, u8) {
        (
            self.cpu.registers.accumulator,
            self.cpu.registers.index_x,
            self.cpu.registers.index_y,
            self.cpu.registers.stack_pointer.0,
        )
    }

    pub fn half_track(&self) -> u8 {
        self.cpu.memory.mechanics.half_track()
    }

    /// Direct access to the internal memory bus (e.g. for inspection or peek/poke).
    pub fn bus(&self) -> &SystemBus<H> {
        &self.cpu.memory
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H> {
        &mut self.cpu.memory
    }
}
