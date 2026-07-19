#![cfg_attr(not(test), no_std)]
extern crate alloc;

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
use pico1541_common::magnetic_surface::MagneticSurface;
use racc6502v4::cpu::CPU;
use systembus::SystemBus;

/// Trait for peeking and poking at memory/registers without side effects.
#[cfg(feature = "debug")]
pub trait MemoryMonitor {
    fn peek<H: pico1541_common::context::ViaHardware, C: pico1541_common::context::CatchUpContext>(&mut self, addr: u16, hw: &mut H, ctx: &mut C) -> u8;
    fn poke<H: pico1541_common::context::ViaHardware, C: pico1541_common::context::CatchUpContext>(&mut self, addr: u16, value: u8, hw: &mut H, ctx: &mut C);
}

pub trait ExecutionTimer {
    fn time_us(&self) -> u32;
    fn syst_current(&self) -> u32;
    fn syst_per_us(&self) -> u32;
    fn yield_cpu(&self);
    fn halt_delay(&self);
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
        emu_debug!("Emulator Reset");
        self.memory.reset(hw);
        self.cpu.reset(&mut self.memory, hw);
        self.memory.mechanics.update_state(self.memory.via2.orb, self.memory.via2.ddrb, hw);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick(&mut self, hw: &mut H) -> u32 {
        #[cfg(feature = "flight-recorder")]
        unsafe {
            crate::flight_recorder::RAM_POINTER = self.memory.memory.as_ptr();
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
                log::info!("FASTLOAD: PC jumped to RAM at ${:04X}!", pc);
            } else {
                log::info!("FASTLOAD: PC returned to ROM at ${:04X}!", pc);
            }
            hw.debug_pin_1(is_ram_execution);
            self.last_ram_execution = is_ram_execution;
        }
        
        let elapsed = self.cpu.step(&mut self.memory, hw);
        
        // Poll IEC and DriveMechanics on every instruction for real-time accuracy without jitter
        self.memory.poll_realtime(hw, elapsed);
        
        if self.cpu.cycles.wrapping_sub(self.memory.next_sync_cycle) < 0x80000000 || self.memory.sync_required {
            self.memory.catch_up(self.cpu.cycles, hw);
            self.memory.sync_required = false;
        }
        
        if self.memory.so_pulsed {
            self.cpu.pending_so = true;
            self.memory.so_pulsed = false;
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

    #[link_section = ".data.ram_func"]
    #[inline(never)]
    pub fn execution_loop<T: ExecutionTimer>(&mut self, hw: &mut H, timer: &T, rom_buffer: &'static [u8; 16384]) -> ! {
        let mut idle_counter = 0u32;
        let mut cycle_counter = 0u32;

        let mut last_tick = timer.time_us();
        let mut was_reset = false;

        let syst_per_us = timer.syst_per_us();
        let mut last_syst = timer.syst_current();
        let mut syst_accum = 0u32;
        let mut now_us = last_tick;

        loop {
            let current_syst = timer.syst_current();
            let diff_syst = last_syst.wrapping_sub(current_syst) & 0x00FFFFFF;
            last_syst = current_syst;
            
            syst_accum += diff_syst;
            while syst_accum >= syst_per_us {
                syst_accum -= syst_per_us;
                now_us = now_us.wrapping_add(1);
            }
            let now = now_us;
            
            if hw.check_halt_requested() {
                timer.halt_delay();
                now_us = timer.time_us();
                last_tick = now_us;
                last_syst = timer.syst_current();
                syst_accum = 0;
                continue;
            }

            if hw.consume_trigger_reset() {
                self.update_rom(rom_buffer);
                self.reset(hw);
            }

            let mut caught_up = 0;
            let is_reset = hw.is_reset_line_asserted();

            while (now.wrapping_sub(last_tick) as i32) > 0 && caught_up < 500 {
                if is_reset {
                    last_tick = last_tick.wrapping_add(1);
                    cycle_counter = cycle_counter.wrapping_add(1);
                    
                    if !was_reset {
                        self.reset(hw);
                        hw.iec_drive(pico1541_common::interface::IecBusState::default());
                        hw.send_event(pico1541_common::interface::DriveEvent::LedChanged(false));
                        hw.send_event(pico1541_common::interface::DriveEvent::MotorChanged(false));
                        was_reset = true;
                    }
                    hw.sync(hw.iec_host());
                } else {
                    if was_reset {
                        self.reset(hw);
                        hw.send_event(pico1541_common::interface::DriveEvent::LedChanged(true));
                        hw.send_event(pico1541_common::interface::DriveEvent::MotorChanged(true));
                        was_reset = false;
                    }
                    
                    let elapsed = self.tick(hw);
                    last_tick = last_tick.wrapping_add(elapsed);
                    cycle_counter = cycle_counter.wrapping_add(elapsed as u32);
                }
                caught_up += 1;
            }
            
            if caught_up == 0 {
                idle_counter = idle_counter.wrapping_add(1);
                timer.yield_cpu();
            }

            // Report exactly on every loop, HostInterface does a fast atomic write
            hw.report_performance_counters(cycle_counter, idle_counter);
        }
    }
}
