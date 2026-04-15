use crossbeam_channel::Sender;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};

/// Snapshot of the three IEC bus lines, used by the debugger to observe bus state.
#[derive(Debug, Clone, Copy, Default)]
pub struct BusSnapshot {
    pub data: bool,
    pub clk: bool,
    pub atn: bool,
}

/// Connects the emulator's HostInterface to the debugger's crossbeam event channel.
/// Separates what the host (C64 side) drives from the combined open-drain bus state.
pub struct HostBridge {
    pub active_track: ActiveTrack,
    event_tx: Sender<DriveEvent>,
    /// What the host (debugger/C64 side) drives: true=asserted/LOW
    pub host_drives: BusSnapshot,
    /// Combined open-drain bus state, updated by drive_data/drive_clk each cycle.
    /// ATN is always derived from host_drives.atn (only the C64 drives ATN).
    pub bus_state: BusSnapshot,
}

impl HostBridge {
    pub fn new(event_tx: Sender<DriveEvent>) -> Self {
        Self {
            active_track: ActiveTrack::new(),
            event_tx,
            host_drives: BusSnapshot::default(),
            bus_state: BusSnapshot::default(),
        }
    }

    /// Returns the C64-side view of the bus.
    /// Use this to assert/release lines as the C64 would, and to read the
    /// combined open-drain state as the C64 would observe it.
    pub fn c64_bus(&mut self) -> C64Bus<'_> { C64Bus(self) }
}

/// The C64's perspective on the IEC bus.
///
/// Implements `IecBus` where `drive_*` asserts lines on behalf of the C64
/// (updating `host_drives` so the emulator sees them via `read_*` on `HostBridge`)
/// and `read_*` returns the combined open-drain state as written back by the
/// emulator each cycle.  ATN is C64-only so it is exposed via `set_atn` rather
/// than through the trait (the 1541 never drives ATN).
pub struct C64Bus<'a>(&'a mut HostBridge);

impl<'a> C64Bus<'a> {
    /// Assert or release ATN (not part of `IecBus` because the 1541 never drives ATN).
    pub fn set_atn(&mut self, assert: bool) {
        self.0.host_drives.atn = assert;
    }
}

impl<'a> IecBus for C64Bus<'a> {
    /// Read the combined (open-drain OR) state of DATA as seen on the bus.
    fn read_data(&self) -> bool { self.0.bus_state.data }
    /// Read the combined (open-drain OR) state of CLK as seen on the bus.
    fn read_clk(&self)  -> bool { self.0.bus_state.clk }
    /// ATN is driven only by the C64; reading it back returns what was last asserted.
    fn read_atn(&self)  -> bool { self.0.host_drives.atn }
    /// Assert or release DATA on behalf of the C64.
    fn drive_data(&mut self, assert: bool) { self.0.host_drives.data = assert; }
    /// Assert or release CLK on behalf of the C64.
    fn drive_clk(&mut self, assert: bool)  { self.0.host_drives.clk = assert; }
}

impl IecBus for HostBridge {
    fn read_data(&self) -> bool { self.host_drives.data }
    fn read_clk(&self)  -> bool { self.host_drives.clk }
    fn read_atn(&self)  -> bool { self.host_drives.atn }
    // Receives the combined open-drain result computed by systembus each cycle.
    fn drive_data(&mut self, assert: bool) {
        self.bus_state.data = assert;
        self.bus_state.atn  = self.host_drives.atn; // ATN is host-only; keep in sync
    }
    fn drive_clk(&mut self, assert: bool) { self.bus_state.clk = assert; }
}

impl HostInterface for HostBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.active_track }
    fn send_event(&mut self, event: DriveEvent) { let _ = self.event_tx.send(event); }
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
