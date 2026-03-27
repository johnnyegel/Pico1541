#![no_std]

pub mod via6522;
pub mod systembus;
pub mod drivemechanics;
pub mod activetrack;
pub mod interface;

use racc6502::cpu::{CPU, CycleState};
use systembus::SystemBus;
use activetrack::ActiveTrack;
use heapless::spsc::Producer;
use log::debug;

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
    /// Returns a reference to the shared track data.
    fn get_active_track(&self) -> &ActiveTrack;

    /// Notifies the manager that the head has moved to a new track.
    fn notify_head_stepped(&mut self, current_half_track: u8);

    /// Notifies the manager that the track buffer has been modified.
    fn notify_track_dirty(&mut self);

    /// Notifies the manager that the LED state has changed.
    fn notify_led_changed(&mut self, enabled: bool);

    /// Notifies the manager that the motor state has changed.
    fn notify_motor_changed(&mut self, enabled: bool);

    /// Get the state of the IEC bus pins (ATN, CLK, DATA).
    fn get_iec_pins(&self) -> u8;

    /// Set the state of the IEC bus pins.
    fn set_iec_pins(&mut self, pins: u8);
}

/// A concrete bridge that implements DriveHardware using an SPSC Queue.
pub struct HardwareBridge {
    /// SPSC Queue: Emu (Producer) sends track change requests -> Manager (Consumer)
    pub command_tx: Producer<'static, Command>,
    
    /// The shared track data (using atomics internally)
    pub track_buffer: &'static ActiveTrack,

    /// Virtual state of IEC pins
    pub iec_pins: u8,
}

impl HardwareBridge {
    pub fn new(track_buffer: &'static ActiveTrack, command_tx: Producer<'static, Command>) -> Self {
        Self {
            track_buffer,
            command_tx,
            iec_pins: 0xFF,
        }
    }
}

impl DriveHardware for HardwareBridge {
    fn get_active_track(&self) -> &ActiveTrack {
        self.track_buffer
    }

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

    fn get_iec_pins(&self) -> u8 {
        self.iec_pins
    }

    fn set_iec_pins(&mut self, pins: u8) {
        self.iec_pins = pins;
    }
}

/// The MOS 1541 Drive Emulator.
pub struct Emulator<H: DriveHardware> {
    pub cpu: CPU<SystemBus<H>>,
}

impl<H: DriveHardware> Emulator<H> {
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
        // Run until we start fetching the next opcode
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
