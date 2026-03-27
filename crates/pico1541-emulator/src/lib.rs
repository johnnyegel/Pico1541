#![no_std]

pub mod via6522;
pub mod systembus;
pub mod drivemechanics;
pub mod interface;

use racc6502::cpu::{CPU, CycleState};
use systembus::SystemBus;
use interface::HostInterface;
use log::debug;

/// The MOS 1541 Drive Emulator.
pub struct Emulator<H: HostInterface> {
    pub cpu: CPU<SystemBus<H>>,
}

impl<H: HostInterface> Emulator<H> {
    /// Constructs a new 1541 Emulator with the provided ROM and hardware interface.
    pub fn new(rom: [u8; 16384], hardware: H) -> Self {
        let bus = SystemBus::new(rom, hardware);
        let cpu = CPU::new(bus);
        Self { cpu }
    }

    /// Resets the 6502 CPU and VIAs.
    pub fn reset(&mut self) {
        debug!("Emulator Reset");
        self.cpu.reset();
    }

    /// Executes a single instruction.
    ///
    /// This will call `tick()` repeatedly until the CPU reaches the next instruction fetch.
    pub fn step(&mut self) {
        loop {
            self.cpu.tick();
            if self.cpu.state == CycleState::FetchOpcode && self.cpu.step_index == 0 {
                break;
            }
        }
    }

    pub fn pc(&self) -> u16 {
        self.cpu.pc
    }

    pub fn half_track(&self) -> u8 {
        self.cpu.bus.mechanics.current_half_track
    }

    /// Direct access to the internal memory bus (e.g. for inspection).
    pub fn bus(&self) -> &SystemBus<H> {
        &self.cpu.bus
    }

    /// Mutable access to the internal memory bus.
    pub fn bus_mut(&mut self) -> &mut SystemBus<H> {
        &mut self.cpu.bus
    }
}
