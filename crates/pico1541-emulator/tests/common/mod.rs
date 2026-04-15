use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};

/// Shared test double for HostInterface.
/// Records drive events; IEC bus lines default to all released (idle).
pub struct TestBridge {
    pub track: ActiveTrack,
    pub events: Vec<DriveEvent>,
    // What the host (C64 side) drives: true = asserted/LOW
    pub host_data: bool,
    pub host_clk: bool,
    pub host_atn: bool,
}

impl TestBridge {
    pub fn new() -> Self {
        Self {
            track: ActiveTrack::new(),
            events: Vec::new(),
            host_data: false,
            host_clk: false,
            host_atn: false,
        }
    }
}

impl IecBus for TestBridge {
    fn read_data(&self) -> bool { self.host_data }
    fn read_clk(&self)  -> bool { self.host_clk }
    fn read_atn(&self)  -> bool { self.host_atn }
    fn drive_data(&mut self, _assert: bool) {}
    fn drive_clk(&mut self, _assert: bool) {}
}

impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, event: DriveEvent) { self.events.push(event); }
}
