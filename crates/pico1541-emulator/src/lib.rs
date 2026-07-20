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

#[export_name = "RAM_POINTER"]
#[used]
pub static mut RAM_POINTER: *const u8 = core::ptr::null();
pub mod hle;
pub mod systembus;
pub mod via6522;

#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
#[allow(unused_imports)]
use pico1541_common::context::CatchUpContext;
use pico1541_common::interface::HostInterface;
use pico1541_common::magnetic_surface::MagneticSurface;
use racc6502v4::cpu::CPU;
use systembus::SystemBus;

/// Trait for peeking and poking at memory/registers without side effects.
#[cfg(feature = "debug")]
pub trait MemoryMonitor {
    fn peek<H: ViaHardware, C: CatchUpContext>(&mut self, addr: u16, hw: &mut H, ctx: &mut C) -> u8;
    fn poke<H: ViaHardware, C: CatchUpContext>(&mut self, addr: u16, value: u8, hw: &mut H, ctx: &mut C);
}

/// The MOS 1541 Drive Emulator.
///
/// `Emulator` holds only the CPU registers and the memory bus (RAM, ROM, VIAs).
/// The `HostInterface` implementation is **not** stored here — callers own it
/// and pass it as `&mut H` to every [`tick`](Self::tick) / [`step`](Self::step) call.
pub struct Emulator<H: HostInterface, S: MagneticSurface> {
    pub cpu: CPU,
    pub memory: SystemBus<H, S>,
    pub cycles: u32,
    last_ram_execution: bool,
}

impl<H: HostInterface, S: MagneticSurface> Emulator<H, S> {
    pub fn new(rom: &'static [u8; 16384], device_id: u8, surface: S) -> Self {
        let bus = SystemBus::new(rom, device_id, surface);
        Self {
            cpu: CPU::new(),
            memory: bus,
            cycles: 0,
            last_ram_execution: false,
        }
    }

    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.memory.update_rom(rom);
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self, hw: &mut H) {
        #[cfg(feature = "defmt")]
        defmt::info!("EMULATOR CORE: Reset called! Defmt is working!");
        emu_debug!("Emulator Reset");
        self.memory.reset(hw);
        self.cpu.reset(&mut self.memory, hw);
        self.last_ram_execution = false;
        self.cycles = self.cpu.cycles;
        self.memory.mechanics.update_state(self.memory.via2.orb, self.memory.via2.ddrb, hw);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        hw.debug_pin_1(true);
        
        #[cfg(feature = "flight-recorder")]
        unsafe {
            crate::flight_recorder::FLIGHT_RECORDER.assume_init_mut().index = 0;
        }

        unsafe {
            crate::RAM_POINTER = self.memory.memory.as_ptr();
        }

        if self.memory.via1.irq || self.memory.via2.irq {
            self.cpu.pending_interrupt = Some(racc6502v4::cpu::Interrupt::Irq);
        }
        let start_cycle = self.cpu.cycles;
        self.memory.current_cycle = start_cycle;
        
        let pc = self.cpu.pc;
        let is_ram_execution = pc < 0x8000;
        if is_ram_execution != self.last_ram_execution {
            if is_ram_execution {
                emu_info!("FASTLOAD: PC jumped to RAM at ${:04X}!", pc);
            } else {
                emu_info!("FASTLOAD: PC returned to ROM at ${:04X}! V1(IFR={:02X},IER={:02X}) V2(IFR={:02X},IER={:02X})", 
                    pc, self.memory.via1.ifr, self.memory.via1.ier, self.memory.via2.ifr, self.memory.via2.ier);
            }
            self.last_ram_execution = is_ram_execution;
        }

        
        let elapsed = self.cpu.step(&mut self.memory, hw);
        
        // Poll IEC and DriveMechanics on every instruction for real-time accuracy without jitter
        self.memory.poll_realtime(hw, elapsed);
        
        hw.debug_pin_1(false);
        if self.cpu.cycles.wrapping_sub(self.memory.next_sync_cycle) < 0x80000000 || self.memory.sync_required {
            self.memory.catch_up(self.cpu.cycles, hw);
            self.memory.sync_required = false;
        }
        
        if self.memory.so_pulsed {
            self.memory.so_pulsed = false;
            self.cpu.pending_so = true;
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
        
        hw.debug_pin_0(false);
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
    pub fn bus(&self) -> &SystemBus<H, S> {
        &self.memory
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H, S> {
        &mut self.memory
    }

}
