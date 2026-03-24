#![no_std]

pub mod via6522;
pub mod systembus;
pub mod drivemechanics;
pub mod activetrack;

#[cfg(test)]
mod test_boot;

use mos6502::cpu::CPU;
use mos6502::instruction::Nmos6502;
use mos6502::registers::Status;
use systembus::SystemBus;
use activetrack::ActiveTrack;
use heapless::spsc::Producer;
use log::info;

/// Commands sent from the Emulator (Core 0) to the Manager (Core 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// Request to load a specific half-track.
    StepHead(u8),
    /// Request to persist the current track back to storage.
    FlushTrack,
    /// Signal that the drive LED state has changed.
    SetLed(bool),
    /// Signal that the motor status has changed.
    SetMotor(bool),
}

/// This trait defines how the Emulator core interacts with the Drive Hardware.
pub trait DriveHardware {
    fn get_active_track(&self) -> &ActiveTrack;
    fn notify_head_stepped(&mut self, current_half_track: u8);
    fn notify_track_dirty(&mut self);
    fn notify_led_changed(&mut self, enabled: bool);
    fn notify_motor_changed(&mut self, enabled: bool);
    fn get_iec_pins(&self) -> u8;
    fn set_iec_pins(&mut self, pins: u8);
}

/// A concrete bridge that implements DriveHardware using an SPSC Queue.
pub struct HardwareBridge {
    pub command_tx: Producer<'static, Command, 16>,
    pub track_buffer: &'static ActiveTrack,
    pub iec_pins: u8,
}

impl HardwareBridge {
    pub fn new(track_buffer: &'static ActiveTrack, command_tx: Producer<'static, Command, 16>) -> Self {
        Self {
            track_buffer,
            command_tx,
            iec_pins: 0xFF,
        }
    }
}

impl DriveHardware for HardwareBridge {
    fn get_active_track(&self) -> &ActiveTrack { self.track_buffer }
    fn notify_head_stepped(&mut self, current_half_track: u8) {
        let _ = self.command_tx.enqueue(Command::StepHead(current_half_track));
    }
    fn notify_track_dirty(&mut self) {
        let _ = self.command_tx.enqueue(Command::FlushTrack);
    }
    fn notify_led_changed(&mut self, enabled: bool) {
        let _ = self.command_tx.enqueue(Command::SetLed(enabled));
    }
    fn notify_motor_changed(&mut self, enabled: bool) {
        let _ = self.command_tx.enqueue(Command::SetMotor(enabled));
    }
    fn get_iec_pins(&self) -> u8 { self.iec_pins }
    fn set_iec_pins(&mut self, pins: u8) { self.iec_pins = pins; }
}

/// The MOS 1541 Drive Emulator.
pub struct Emulator<H: DriveHardware> {
    cpu: CPU<SystemBus<H>, Nmos6502>,
    last_cpu_cycles: u64,
}

impl<H: DriveHardware> Emulator<H> {
    pub fn new(rom: [u8; 16384], hardware: H) -> Self {
        let bus = SystemBus::new(rom, hardware);
        let cpu = CPU::new(bus, Nmos6502::default());
        Self { cpu, last_cpu_cycles: 0 }
    }

    pub fn reset(&mut self) {
        info!("Emulator Reset");
        self.cpu.reset();
        self.last_cpu_cycles = self.cpu.cycles;
    }

    pub fn step(&mut self) {
        self.cpu.single_step();

        let current_cycles = self.cpu.cycles;
        let delta = current_cycles.wrapping_sub(self.last_cpu_cycles);
        self.last_cpu_cycles = current_cycles;

        let memory_ticks = self.cpu.memory.ticks_this_step;
        self.cpu.memory.ticks_this_step = 0;

        if delta > memory_ticks as u64 {
            let extra_ticks = delta - memory_ticks as u64;
            for _ in 0..extra_ticks {
                self.cpu.memory.tick_cycle();
            }
        }

        if self.cpu.memory.so_pending {
            self.cpu.memory.so_pending = false;
            self.cpu.registers.status.insert(Status::PS_OVERFLOW);
        }
    }

    pub fn pc(&self) -> u16 { self.cpu.registers.program_counter }
    pub fn half_track(&self) -> u8 { self.cpu.memory.mechanics.current_half_track }
    pub fn bus(&self) -> &SystemBus<H> { &self.cpu.memory }
    pub fn bus_mut(&mut self) -> &mut SystemBus<H> { &mut self.cpu.memory }
}
