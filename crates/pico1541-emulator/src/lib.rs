#![cfg_attr(not(test), no_std)]
extern crate alloc;

#[allow(unused_macros)]
#[cfg(all(not(feature = "logging"), not(feature = "defmt")))]
macro_rules! emu_info { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(all(not(feature = "logging"), not(feature = "defmt")))]
macro_rules! emu_warn { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(all(not(feature = "logging"), not(feature = "defmt")))]
macro_rules! emu_debug { ($($arg:tt)*) => {} }

#[allow(unused_macros)]
#[cfg(all(feature = "logging", not(feature = "defmt")))]
macro_rules! emu_info { ($($arg:tt)*) => { log::info!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(all(feature = "logging", not(feature = "defmt")))]
macro_rules! emu_warn { ($($arg:tt)*) => { log::warn!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(all(feature = "logging", not(feature = "defmt")))]
macro_rules! emu_debug { ($($arg:tt)*) => { log::debug!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(feature = "defmt")]
macro_rules! emu_info { ($($arg:tt)*) => { defmt::info!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(feature = "defmt")]
macro_rules! emu_warn { ($($arg:tt)*) => { defmt::warn!($($arg)*) } }

#[allow(unused_macros)]
#[cfg(feature = "defmt")]
macro_rules! emu_debug { ($($arg:tt)*) => { defmt::debug!($($arg)*) } }

pub mod drivemechanics;
#[cfg(feature = "flight-recorder")]
pub mod flight_recorder;

pub mod hle;
pub mod memory;
pub mod systembus;
pub mod time;
pub mod via6522;

#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::HostInterface;
use pico1541_common::magnetic_surface::MagneticSurface;
use racc6502v4::cpu::CPU;
use systembus::SystemBus;
pub use crate::drivemechanics::DriveMechanics;

/// Trait for peeking and poking at memory/registers without side effects.
#[cfg(feature = "debug")]
pub trait MemoryMonitor {
    fn peek<H: ViaHardware>(&mut self, addr: u16, hw: &mut H) -> u8;
    fn poke<H: ViaHardware>(&mut self, addr: u16, value: u8, hw: &mut H);
}

/// The MOS 1541 Drive Emulator.
///
/// `Emulator` holds only the CPU registers and the memory bus (RAM, ROM, VIAs).
/// The `HostInterface` implementation is **not** stored here — callers own it
/// and pass it as `&mut H` to every [`tick`](Self::tick) / [`step`](Self::step) call.
pub struct Emulator<H: HostInterface, S: MagneticSurface> {
    pub cpu: CPU,
    pub systembus: SystemBus<H, S>,
    pub cycles: u32,
}

impl<H: HostInterface, S: MagneticSurface> Emulator<H, S> {
    pub fn new(
        rom: &'static [u8; 16384],
        memory: &'static mut [u8; 32768],
        device_id: u8,
        mechanics: DriveMechanics<S>,
    ) -> Self {
        let bus = SystemBus::new(rom, memory, device_id, mechanics);
        Self {
            cpu: CPU::new(),
            systembus: bus,
            cycles: 0,
        }
    }

    /// Updates the ROM image used by the emulator.
    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.systembus.update_rom(rom);
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self, hw: &mut H) {
        self.cpu.reset(&mut self.systembus, hw);
        self.systembus.reset(hw);
        self.cycles = self.cpu.cycles;
    }


    #[link_section = ".data.pico1541_emulator_core"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        
        #[cfg(feature = "flight-recorder")]
        unsafe {
            crate::flight_recorder::FLIGHT_RECORDER.assume_init_mut().index = 0;
        }

        if self.systembus.has_irq() {
            self.cpu.pending_interrupt = Some(racc6502v4::cpu::Interrupt::Irq);
        }
        
        let elapsed = self.cpu.step(&mut self.systembus, hw);
        
        // Do all internal emulation math (timers, mechanics) while we have free time
        self.systembus.catch_up(self.cpu.cycles);
        
        // Wait until physical real-time matches the simulated cpu.cycles
        self.systembus.time.sync_realtime(self.cpu.cycles, hw);

        // Now that we are perfectly synchronized in time, sample physical pins!
        self.systembus.poll_io(hw, elapsed, self.cpu.cycles);

        if self.systembus.so_pending() {
            self.cpu.pending_so = true;
        }

        // Flight recorder re-enabled, optimized to waste minimal cycles
        #[cfg(feature = "flight-recorder")]
        unsafe {
            let ptr = core::ptr::addr_of_mut!(crate::flight_recorder::FLIGHT_RECORDER) as *mut crate::flight_recorder::FlightRecorder;
            let current_via1 = self.systembus.via1.pb_in();
            
            // Inline the check to avoid function call overhead and branch prediction misses for non-events
            if (*ptr).last_record.via1_pb != current_via1 {
                let is_sync = (self.systembus.via2.pb_in() & 0x80) == 0;
                (*ptr).record(crate::flight_recorder::FlightRecord {
                    cycle: self.cpu.cycles,
                    pc: self.cpu.pc as u32,
                    event_type: 0,
                    via1_pb: current_via1,
                    via2_pb: self.systembus.via2.pb_in(),
                    pcr1: self.systembus.via1.pcr(),
                    pcr2: self.systembus.via2.pcr(),
                    via2_pa: self.systembus.via2.pa_in(),
                    is_sync: if is_sync { 1 } else { 0 },
                    _padding: 0,
                });
            }
        }
        self.cycles = self.cpu.cycles;
        elapsed as u32
    }

    /// Executes the given number of CPU cycles.
    pub fn run_for(&mut self, hw: &mut H, cycles: u32) {
        let start = self.cycles;
        while self.cycles.wrapping_sub(start) < cycles {
            self.tick(hw);
        }
    }

    pub fn step(&mut self, hw: &mut H) {
        self.tick(hw);
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn state(&self) -> (u8, u8, u8, u8) {
        (
            self.cpu.a,
            self.cpu.x,
            self.cpu.y,
            self.cpu.s,
        )
    }

    pub fn half_track(&self) -> u8 {
        self.systembus.mechanics.half_track()
    }

    /// Direct access to the internal memory bus (e.g. for inspection or peek/poke).
    pub fn bus(&self) -> &SystemBus<H, S> {
        &self.systembus
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H, S> {
        &mut self.systembus
    }

}
