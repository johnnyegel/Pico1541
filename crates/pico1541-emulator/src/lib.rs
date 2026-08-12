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
pub mod systembus;
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
    pub fn new(rom: &'static [u8; 16384], device_id: u8, mechanics: DriveMechanics<S>) -> Self {
        let bus = SystemBus::new(rom, device_id, mechanics);
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

    /// Initializes the emulator, resetting the CPU and VIAs.
    pub fn initialize(&mut self, hw: &mut H) {
        self.cpu.reset(&mut self.systembus, hw);
        self.systembus.reset(hw);
        self.cycles = self.cpu.cycles;
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self, hw: &mut H) {
        // Reset the CPU and VIAs, and re-initialize the drive mechanics.
        self.initialize(hw);

        // Set the VIA2 Port B outputs to 0xFF, which is the default state after a reset.
        hw.write_mechanics_port(0xFF);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        
        #[cfg(feature = "flight-recorder")]
        unsafe {
            crate::flight_recorder::FLIGHT_RECORDER.assume_init_mut().index = 0;
        }

        if self.systembus.via1.irq || self.systembus.via2.irq {
            self.cpu.pending_interrupt = Some(racc6502v4::cpu::Interrupt::Irq);
        }
        
        self.systembus.iec_synced_this_instruction = false;
        let elapsed = self.cpu.step(&mut self.systembus, hw);
        
        // Poll IEC and DriveMechanics on every instruction for real-time accuracy without jitter
        self.systembus.poll_realtime(hw, elapsed);
        
        if self.cpu.cycles.wrapping_sub(self.systembus.next_sync_cycle) < 0x80000000 {
            hw.spin_until(self.cpu.cycles);
        }
        hw.clear_debug_pins();

        self.systembus.catch_up(self.cpu.cycles);
        
        if self.systembus.so_pulsed {
            self.systembus.so_pulsed = false;
            self.cpu.pending_so = true;
            hw.set_debug_0(); // Output SO pulse to logic analyzer
        }

        // Flight recorder re-enabled, optimized to waste minimal cycles
        #[cfg(feature = "flight-recorder")]
        unsafe {
            let ptr = core::ptr::addr_of_mut!(crate::flight_recorder::FLIGHT_RECORDER) as *mut crate::flight_recorder::FlightRecorder;
            let is_sync = (self.memory.via2.in_b & 0x80) == 0;
            (*ptr).record(crate::flight_recorder::FlightRecord {
                cycle: self.cpu.cycles,
                pc: self.cpu.pc as u32,
                event_type: 0,
                via1_pb: self.memory.via1.in_b,
                via2_pb: self.memory.via2.in_b,
                pcr1: self.memory.via1.pcr,
                pcr2: self.memory.via2.pcr,
                via2_pa: self.memory.via2.in_a,
                is_sync: if is_sync { 1 } else { 0 },
                _padding: 0,
            });
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
