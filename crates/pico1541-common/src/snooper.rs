use crate::interface::{IecBusState, DriveEvent};
use portable_atomic::{AtomicU8, Ordering, AtomicBool};

pub struct SnooperConfig {
    pub device_id: AtomicU8,
    pub listen_addr: AtomicU8,
    pub talk_addr: AtomicU8,
    pub hijack_active: AtomicBool,
    pub discard_request: AtomicBool,
    pub should_drive_data: AtomicBool,
    pub emu_speed_hz: portable_atomic::AtomicU32,
    pub core0_avg_raw: portable_atomic::AtomicU32,
    pub core0_max_raw: portable_atomic::AtomicU32,
    pub core1_avg_raw: portable_atomic::AtomicU32,
    pub core1_max_raw: portable_atomic::AtomicU32,
    pub diag_old_tick: portable_atomic::AtomicU32,
    pub diag_startup_ticks: portable_atomic::AtomicU32,
    pub trigger_reset: AtomicBool,
    pub halt_emulator: AtomicBool,
}

impl SnooperConfig {
    pub const fn new(device_id: u8) -> Self {
        Self {
            device_id: AtomicU8::new(device_id),
            listen_addr: AtomicU8::new(0x20 | (device_id & 0xF)),
            talk_addr: AtomicU8::new(0x40 | (device_id & 0xF)),
            hijack_active: AtomicBool::new(false),
            discard_request: AtomicBool::new(false),
            should_drive_data: AtomicBool::new(false),
            emu_speed_hz: portable_atomic::AtomicU32::new(0),
            core0_avg_raw: portable_atomic::AtomicU32::new(0),
            core0_max_raw: portable_atomic::AtomicU32::new(0),
            core1_avg_raw: portable_atomic::AtomicU32::new(0),
            core1_max_raw: portable_atomic::AtomicU32::new(0),
            diag_old_tick: portable_atomic::AtomicU32::new(0),
            diag_startup_ticks: portable_atomic::AtomicU32::new(0),
            trigger_reset: AtomicBool::new(false),
            halt_emulator: AtomicBool::new(false),
        }
    }

    pub fn update_device_id(&self, device_id: u8) {
        self.device_id.store(device_id, Ordering::Relaxed);
        self.listen_addr.store(0x20 | (device_id & 0xF), Ordering::Relaxed);
        self.talk_addr.store(0x40 | (device_id & 0xF), Ordering::Relaxed);
    }
}

/// Struct to represent edges on the IEC bus
struct IecBusEdge {
    state: IecBusState,
    delta: IecBusState,
}

impl IecBusEdge {
    /// Returns true if the clock line has an edge matching the expected level.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn is_clk(&self, expect: bool) -> bool {
        self.delta.get_clk() && self.state.get_clk() == expect
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum SnooperState {
    Idle,
    Receiving,
    WaitTurnaround,
} 

/// Implements a snooper that monitors the IEC bus lines and decodes commands sent to the drive
pub struct IecSnooper {
    listen_addr: u8,

    last_iec: IecBusState,
    state: SnooperState,
    bit_count: u8,
    current_byte: u8,
    is_eoi: bool,
    is_atn: bool,

    is_listening: bool,
    secondary_address: u8
}

impl IecSnooper {

    pub fn new(device_id: u8) -> Self {
        Self {
            listen_addr: 0x20 | (device_id & 0xF),
            last_iec: IecBusState::default(),
            state: SnooperState::Idle,
            bit_count: 0,
            current_byte: 0,
            is_eoi: false,
            is_atn: false,
            is_listening: false,
            secondary_address: 0,
        }
    }

    pub fn set_listen_addr(&mut self, listen_address: u8) {
        self.listen_addr = listen_address;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn update<F: FnMut(DriveEvent)>(&mut self, iec_state: IecBusState, mut dispatch: F) {
        // Detect edge changed and return early if no change
        let delta = iec_state.delta(&self.last_iec);
        if delta.0 == 0 {
            return
        }

        // Capture the edge information, and update the last known state
        let edge = IecBusEdge { state: iec_state, delta };
        self.last_iec = iec_state;

        // :? Is this an ATN edge?
        if delta.get_atn() {
            // -> Yes, then reset to IDLE
            if iec_state.get_atn() && self.is_listening {
                // DATA phase ended by host pulling ATN low
                dispatch(DriveEvent::IecSnoopEndOfTransaction);
            }
            self.state = SnooperState::Idle;
            return;
        }

        // Start of data: CLK edge release, DATA asserted
        match self.state {
            SnooperState::Idle => {
                // Talker pulls CLK low (True) to signal start of a byte transfer
                if edge.is_clk(true) {
                    // NOTE! EOI detection requires timing (200us pause) or seeing drive lines.
                    // Since we only see host lines and have no timer here, we rely on 
                    // ATN/UNLISTEN transitions to finalize commands.
                    self.is_eoi = false; 
                    self.is_atn = iec_state.get_atn();
                    
                    self.state = SnooperState::Receiving;
                    self.bit_count = 0;
                    self.current_byte = 0;
                }
            }
            SnooperState::Receiving => {
                // Bits are valid on the rising edge of CLK (Release/False)
                if edge.is_clk(false) {
                    // Capture the bit value (inverted logic: Released=1, Asserted=0)
                    let bit = if !iec_state.get_data() { 1 } else { 0 };
                    self.current_byte |= bit << self.bit_count;
                    self.bit_count += 1;

                    // :? Full byte received?
                    if self.bit_count == 8 {
                        // -> Yes, process the byte
                        self.dispatch_byte(self.current_byte, self.is_atn, self.is_eoi, &mut dispatch);
                        self.state = SnooperState::WaitTurnaround;
                    }
                }
            }
            SnooperState::WaitTurnaround => {
                // After 8 bits, the talker pulls CLK low again (True) to wait for ACK.
                // We wait for this falling edge before going back to Idle.
                if edge.is_clk(true) {
                    self.state = SnooperState::Idle;
                }
            }
        }
    }

    #[link_section = ".data.ram_func"]
    fn dispatch_byte<F: FnMut(DriveEvent)>(&mut self, byte: u8, atn: bool, eoi: bool, dispatch: &mut F) {
        log::debug!("[Snooper] Received byte 0x{:02X} (ATN={}, EOI={})", byte, atn, eoi);
        // :? Is this an ATN byte?
        if atn {
            // :? Is it a secondary address (0x60..0x7F)?
            if (byte & 0xE0) == 0x60 {
                if self.is_listening {
                    self.secondary_address = byte;
                    dispatch(DriveEvent::IecSnoopSecondaryAddress(byte));
                }
            } else if byte == self.listen_addr {
                // -> No, but this byte is our listen address, so we should start listening
                self.is_listening = true;
            } else if byte == 0x3F { // UNLISTEN
                if self.is_listening {
                    dispatch(DriveEvent::IecSnoopEndOfTransaction);
                }
                self.is_listening = false;
            }
            return;
        }

        // E -> No, are we listening?
        if !self.is_listening {
            // -> No, then we ignore this byte
            return;
        }

        // E -> Yes, then this is a data byte we should capture
        dispatch(DriveEvent::IecSnoopByteReceived(byte, eoi));

        if eoi {
            self.is_listening = false;
        }
    }
}

pub struct IecSnoopByteHandler {
    buffer: [u8; 32],
    pos: usize,
    secondary_address: u8,
    hijack_active: bool,
    eoi_received: bool,
    cmd_len: usize,
}

impl IecSnoopByteHandler {
    pub fn new() -> Self {
        Self {
            buffer: [0; 32],
            pos: 0,
            secondary_address: 0,
            hijack_active: false,
            eoi_received: false,
            cmd_len: 0,
        }
    }
}

impl Default for IecSnoopByteHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl IecSnoopByteHandler {
    #[link_section = ".data.ram_func"]
    pub fn handle(&mut self, event: DriveEvent, mut dispatch: &mut dyn FnMut(DriveEvent)) {
        match event {
            DriveEvent::IecSnoopSecondaryAddress(sa) => {
                self.pos = 0;
                self.cmd_len = 0;
                self.secondary_address = sa;
                self.hijack_active = false;
                self.eoi_received = false;
            }
            DriveEvent::IecSnoopByteReceived(byte, eoi) => {
                if self.pos < self.buffer.len() {
                    self.buffer[self.pos] = byte;
                    self.pos += 1;
                }

                // Check for hijack condition: starts with "CD:"
                if self.pos >= 3 && &self.buffer[0..3] == b"CD:" {
                    self.hijack_active = true;
                }

                if eoi {
                    self.finish_command(&mut dispatch);
                }
            }
            DriveEvent::IecSnoopEndOfTransaction => {
                self.finish_command(&mut dispatch);
            }
            _ => {}
        }
    }

    fn finish_command(&mut self, mut dispatch: impl FnMut(DriveEvent)) {
        if self.pos > 0 {
            // Emit IecCommand if it's on the command channel (SA 15) or hijacked
            let sa = self.secondary_address & 0x0F;
            let is_dir_command = self.pos > 0 && self.buffer[0] == b'$';
            if sa == 15 || self.hijack_active || is_dir_command {
                let mut data = [0u8; 32];
                let len = self.pos.min(32);
                data[..len].copy_from_slice(&self.buffer[..len]);
                dispatch(DriveEvent::IecCommand { data, len: len as u8 });
            }
            self.cmd_len = self.pos;
            self.pos = 0; // Clear for next command
            self.eoi_received = true;
        }
    }

    pub fn is_hijacked(&self) -> bool {
        self.hijack_active
    }

    pub fn should_discard(&self) -> bool {
        self.hijack_active && self.eoi_received
    }

    pub fn get_command(&self) -> Option<&str> {
        let len = if self.pos > 0 { self.pos } else { self.cmd_len };
        if len > 0 {
            core::str::from_utf8(&self.buffer[..len]).ok()
        } else {
            None
        }
    }
}
