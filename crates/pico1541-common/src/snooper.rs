
use crate::interface::{IecBusState, DriveEvent};

/// Struct to represent edges on the IEC bus
struct IecBusEdge {
    state: IecBusState,
    delta: IecBusState,
}

impl IecBusEdge {
    /// Returns true if the clock line has an edge matching the expected level.
    #[inline]
    pub fn is_clk(&self, expect: bool) -> bool {
        self.delta.get_clk() && self.state.get_clk() == expect
    }

    /// Returns true if the data line has an edge matching the expected level.
    #[inline]
    pub fn is_data(&self, expect: bool) -> bool {
        self.delta.get_data() && self.state.get_data() == expect
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum SnooperState {
    Idle,
    Ready,
    Receiving,
    Ack,
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
            self.state = SnooperState::Idle;
            log::trace!("[Snooper] ATN changed to {}, returning to IDLE", iec_state.get_atn());
            return;
        }

        // Start of data: CLK edge release, DATA asserted
        match self.state {
            SnooperState::Idle => {
                if edge.is_data(false) {
                    self.state = SnooperState::Ready;
                    self.is_eoi = false;
                    log::trace!("[Snooper] Detected start condition (CLK released, DATA released)");
                }
            }
            SnooperState::Ready => {
                // :? Is data asserted without clock?
                if edge.is_data(false) {
                    // -> Yes, We have an EOI
                    self.is_eoi = true;
                    log::trace!("[Snooper] Detected DATA released in Ready. EOI detected.");
                    return;
                }
                // :? Is the Clock asserted?
                if edge.is_clk(true) {
                    // -> Yes, the we are in the receiving state
                    self.state = SnooperState::Receiving;
                    self.bit_count = 0;
                    self.current_byte = 0;
                    self.is_atn = iec_state.get_atn();
                    log::trace!("[Snooper] Detected CLK asserted, ATN={}, entering RECEIVING state", self.is_atn);

                }
            }
            SnooperState::Receiving => {
                // Handle receiving state logic
                if edge.is_clk(false) {
                    // On clock release, capture the bit value
                    let bit = if !iec_state.get_data() { 1 } else { 0 };
                    self.current_byte |= bit << self.bit_count;
                    self.bit_count += 1;
                    log::trace!("[Snooper] Captured bit {}: {} (DATA={})", self.bit_count - 1, bit, iec_state.get_data());

                    // :? Full byte received?
                    if self.bit_count == 8 {
                        // -> Yes, process the byte
                        self.dispatch_byte(self.current_byte, self.is_atn, self.is_eoi, &mut dispatch);
                        self.state = SnooperState::Ack;
                    }
                }
            }
            SnooperState::Ack => {
                // Handle ack state logic
                if edge.is_data(true) {
                    // On data assert, we go back to idle
                    self.state = SnooperState::Idle;
                    log::trace!("[Snooper] Detected ACK (DATA asserted), returning to IDLE");
                }
            }
        }
    }

    fn dispatch_byte<F: FnMut(DriveEvent)>(&mut self, byte: u8, atn: bool, eoi: bool, dispatch: &mut F) {
        log::trace!("[Snooper] Decoded byte: 0x{:02X}, ATN: {}, EOI: {}", byte, atn, eoi);

        // :? Is this an ATN byte?
        if atn {
            // -> Yes :? Are we already listening?
            if self.is_listening {
                self.secondary_address = byte;
                log::trace!("[Snooper] Received secondary address byte: 0x{:02X}", byte);
                dispatch(DriveEvent::IecSnoopSecondaryAddress(byte));
            }
            else if byte == self.listen_addr {
                // -> No, but this byte is the listen address, so we should start listening
                self.is_listening = true;
                log::trace!("[Snooper] Device addressed with byte: 0x{:02X}, entering listening mode", byte);
            }
            return;
        }

        // E -> No, are we listening?
        if !self.is_listening {
            // -> No, then we ignore this byte
            return;
        }

        // E -> Yes, then this is a data byte we should capture
        log::debug!("[Snooper] Received data byte: 0x{:02X} while listening, EOI={}", byte, eoi);
        dispatch(DriveEvent::IecSnoopByteReceived(byte, eoi));

        if eoi {
            log::trace!("[Snooper] EOI detected on byte: 0x{:02X}. Stop listening.", byte);
            self.is_listening = false;
        }
    }
}

pub struct IecSnoopByteHandler {
    buffer: [u8; 32],
    pos: usize,
    command_detected: bool,
    eoi_received: bool,
}

impl IecSnoopByteHandler {
    pub fn new() -> Self {
        Self {
            buffer: [0; 32],
            pos: 0,
            command_detected: false,
            eoi_received: false,
        }
    }

    pub fn handle(&mut self, event: DriveEvent) {
        match event {
            DriveEvent::IecSnoopSecondaryAddress(_) => {
                self.pos = 0;
                self.command_detected = false;
                self.eoi_received = false;
            }
            DriveEvent::IecSnoopByteReceived(byte, eoi) => {
                if self.pos < self.buffer.len() {
                    self.buffer[self.pos] = byte;
                    self.pos += 1;

                    if self.pos == 3 {
                        if &self.buffer[0..3] == b"CD:" {
                            self.command_detected = true;
                        }
                    }
                }
                if eoi {
                    self.eoi_received = true;
                }
            }
            _ => {}
        }
    }

    pub fn is_hijacked(&self) -> bool {
        self.command_detected
    }

    pub fn should_discard(&self) -> bool {
        self.command_detected && self.eoi_received
    }

    pub fn get_command(&self) -> Option<&str> {
        if self.command_detected {
            core::str::from_utf8(&self.buffer[..self.pos]).ok()
        } else {
            None
        }
    }
}

