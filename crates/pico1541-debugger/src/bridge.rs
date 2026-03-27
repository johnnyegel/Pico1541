use crossbeam_channel::Sender;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};

/// Connects the emulator's HostInterface to the debugger's crossbeam event channel.
/// Separates what the host (debugger) drives from the combined open-drain bus state.
pub struct HostBridge {
    pub active_track: ActiveTrack,
    event_tx: Sender<DriveEvent>,
    /// What the host (debugger/C64 side) drives: 1=HIGH/released, 0=LOW/asserted
    pub host_drives: IecBus,
    /// Combined open-drain bus state, written back by emulator each cycle
    pub bus_state: IecBus,
}

impl HostBridge {
    pub fn new(event_tx: Sender<DriveEvent>) -> Self {
        let all_released = IecBus(IecBus::DATA | IecBus::CLK | IecBus::ATN);
        Self {
            active_track: ActiveTrack::new(),
            event_tx,
            host_drives: all_released,
            bus_state: all_released,
        }
    }
}

impl HostInterface for HostBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.active_track }
    fn send_event(&mut self, event: DriveEvent) { let _ = self.event_tx.send(event); }
    fn read_iec(&self) -> IecBus { self.host_drives }
    fn write_iec(&mut self, pins: IecBus) { self.bus_state = pins; }
}

// ─── Debugger commands and responses ─────────────────────────────────────────

#[derive(Debug)]
pub enum DebugCommand {
    Run,
    Pause,
    Step,
    Reset,
    SetBreakpoint(u16),
    ClearBreakpoint(u16),
    ReadMemory(u16, u16),
    Mount(String),
    Unmount,
    IECCommand(String),
    Exit,
}

#[derive(Debug, Clone)]
pub struct DisasmLine {
    pub addr: u16,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct CpuContext {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub p: u8,
    pub ht: u8,
    pub led: bool,
    pub motor: bool,
    pub disasm_window: Vec<DisasmLine>,
}

#[derive(Debug)]
pub enum DebugResponse {
    Context(CpuContext),
    BreakpointHit(CpuContext),
    DriveEvent(String),
    Memory(u16, Vec<u8>),
    IECResponse(String),
}
