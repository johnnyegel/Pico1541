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
#[cfg(feature = "flight-recorder")]
pub mod flight_recorder;
pub mod hle;
pub mod systembus;
pub mod via6522;

use pico1541_common::interface::HostInterface;
use racc6502v3::CPU;
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
    pub cpu: CPU,
    pub memory: SystemBus<H>,
    pub cycles: u32,
}

impl<H: HostInterface> Emulator<H> {
    pub fn new(rom: &'static [u8; 16384], device_id: u8, track_handle: pico1541_common::track_buffer::EmulatorHandle) -> Self {
        let bus = SystemBus::new(rom, device_id, track_handle);
        Self {
            cpu: CPU::new(),
            memory: bus,
            cycles: 0,
        }
    }

    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.memory.update_rom(rom);
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self, hw: &mut H) {
        emu_debug!("Emulator Reset");
        self.memory.reset(hw);
        self.cpu.reset(&mut self.memory, hw);
        self.memory.mechanics.update_state(self.memory.via2.orb, self.memory.via2.ddrb, hw);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        if self.memory.via1.irq || self.memory.via2.irq {
            self.cpu.pending_interrupt = Some(racc6502v3::cpu::Interrupt::Irq);
        }
        if self.memory.so_pulsed {
            self.cpu.pending_so = true;
            self.memory.so_pulsed = false;
        }
        
        let start_cycle = self.cpu.cycles;
        let elapsed = self.cpu.step(&mut self.memory, hw);
        self.cpu.cycles = start_cycle.wrapping_add(elapsed as u32);
        
        self.memory.catch_up(self.cpu.cycles, hw);
        
        if self.memory.so_pulsed {
            self.cpu.pending_so = true;
            self.memory.so_pulsed = false;
        }
        
        // Flight recorder re-enabled, optimized to waste minimal cycles
        #[cfg(feature = "flight-recorder")]
        unsafe {
            let ptr = core::ptr::addr_of_mut!(crate::flight_recorder::FLIGHT_RECORDER) as *mut crate::flight_recorder::FlightRecorder;
            (*ptr).record(crate::flight_recorder::FlightRecord {
                cycle: self.cpu.cycles,
                pc: self.cpu.pc as u32,
                event_type: 0,
                via1_pb: self.memory.via1.in_b,
                via2_pb: self.memory.via2.in_b,
                pcr1: self.memory.via1.pcr,
                pcr2: self.memory.via2.pcr,
                _padding: [0; 3],
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
        self.memory.mechanics.half_track()
    }

    /// Direct access to the internal memory bus (e.g. for inspection or peek/poke).
    pub fn bus(&self) -> &SystemBus<H> {
        &self.memory
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H> {
        &mut self.memory
    }
}
