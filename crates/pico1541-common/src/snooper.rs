use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Represents the IEC bus state as mirrored in VIA1 Port B.
///
/// Layout:
/// - Bit 0 (0x01): DATA (true = Asserted/LOW)
/// - Bit 2 (0x04): CLK (true = Asserted/LOW)
/// - Bit 7 (0x80): ATN (true = Asserted/LOW)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IecBusByte(pub u8);

impl IecBusByte {
    pub const MASK: u8 = 0x85; // 0b1000_0101

    #[inline]
    pub fn from_pb(pb: u8) -> Self {
        Self(pb & Self::MASK)
    }

    #[inline]
    pub fn data(&self) -> bool {
        (self.0 & 0x01) != 0
    }

    #[inline]
    pub fn clk(&self) -> bool {
        (self.0 & 0x04) != 0
    }

    #[inline]
    pub fn atn(&self) -> bool {
        (self.0 & 0x80) != 0
    }

    /// Returns a byte where bits are set if they have transitioned since `prev`.
    #[inline]
    pub fn transitions(&self, prev: Self) -> u8 {
        self.0 ^ prev.0
    }
}

/// Shared configuration and status for the snooper, allowing cross-core coordination.
pub struct SnooperConfig {
    pub hijack_active: AtomicBool,
    pub should_drive_data: AtomicBool,
    pub discard_request: AtomicBool,
    pub listen_addr: AtomicU8,
    pub talk_addr: AtomicU8,
}

impl SnooperConfig {
    pub const fn new(device_id: u8) -> Self {
        Self {
            hijack_active: AtomicBool::new(false),
            should_drive_data: AtomicBool::new(false),
            discard_request: AtomicBool::new(false),
            listen_addr: AtomicU8::new(0x20 | device_id),
            talk_addr: AtomicU8::new(0x40 | device_id),
        }
    }

    pub fn reset(&self, device_id: u8) {
        self.hijack_active.store(false, Ordering::Relaxed);
        self.should_drive_data.store(false, Ordering::Relaxed);
        self.discard_request.store(false, Ordering::Relaxed);
        self.listen_addr.store(0x20 | device_id, Ordering::Relaxed);
        self.talk_addr.store(0x40 | device_id, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnooperState {
    Idle,
    Ready,
    Receiving,
    Ack,
}

pub struct IecSnooper {
    state: SnooperState,
    last_state: IecBusByte,

    // Layer 1: Byte Decoder State
    bit_count: u8,
    current_byte: u8,
    atn_at_start: bool,

    // Layer 2: Protocol State
    device_addressed: bool,
    channel_15_active: bool,

    // Layer 3: Command Buffer
    buffer: [u8; 64],
    buffer_len: usize,

    pub command_ready: bool,
    ready_delay: u8,
    was_hijacked: bool,
    clk_released: bool,

    // Sync state
    pub config: &'static SnooperConfig,
}

impl IecSnooper {
    pub fn new(_device_id: u8, config: &'static SnooperConfig) -> Self {
        Self {
            state: SnooperState::Idle,
            last_state: IecBusByte(0),
            bit_count: 0,
            current_byte: 0,
            atn_at_start: false,
            device_addressed: false,
            channel_15_active: false,
            buffer: [0; 64],
            buffer_len: 0,
            command_ready: false,
            ready_delay: 0,
            was_hijacked: false,
            clk_released: false,
            config,
        }
    }

    pub fn tick(&mut self, state: IecBusByte, delta: u32) -> Option<[u8; 64]> {
        let atn_edge_pos = state.atn() && !self.last_state.atn();
        let clk_edge_pos = state.clk() && !self.last_state.clk();
        let clk_edge_neg = !state.clk() && self.last_state.clk();
        let data_edge_neg = !state.data() && self.last_state.data();

        // If ATN is pulled while we are waiting for a byte, mark it as an ATN byte.
        if atn_edge_pos {
            self.atn_at_start = true;
        }

        let hijack_active = self.config.hijack_active.load(Ordering::Relaxed);

        // --- Layer 1: Byte Decoder ---
        match self.state {
            SnooperState::Idle => {
                // Enter Ready on DATA pull (standard start of byte)
                // If hijacked, also enter Ready on CLK/ATN transitions to stay in sync.
                if data_edge_neg || (hijack_active && (clk_edge_neg || atn_edge_pos)) {
                    self.atn_at_start = state.atn();
                    self.state = SnooperState::Ready;
                    self.ready_delay = 20;
                    self.clk_released = false;
                }
            }
            SnooperState::Ready => {
                if !state.clk() {
                    self.clk_released = true;
                }

                if self.ready_delay > 0 {
                    self.ready_delay = self.ready_delay.saturating_sub(delta.min(255) as u8);
                }

                // Transition to Receiving on the leading edge of the first bit pulse (CLK Release)
                if clk_edge_neg && self.clk_released {
                    self.state = SnooperState::Receiving;
                    self.bit_count = 0;
                    self.current_byte = 0;
                }
            }
            SnooperState::Receiving => {
                // Read bits on the trailing edge of the pulse (CLK Assert)
                if clk_edge_pos {
                    // Use last_state.data to get the data that was stable during the pulse.
                    let bit = if !self.last_state.data() { 1 } else { 0 };
                    self.current_byte |= bit << self.bit_count;
                    self.bit_count += 1;

                    if self.bit_count == 8 {
                        self.process_byte(self.current_byte, self.atn_at_start);
                        self.state = SnooperState::Ack;
                    }
                }
            }
            SnooperState::Ack => {
                // Return to Idle on CLK release (end of byte handshake)
                if clk_edge_neg {
                    self.state = SnooperState::Idle;
                }
            }
        }
        self.last_state = state;

        // Update shared status
        self.config
            .should_drive_data
            .store(self.should_drive_data(), Ordering::Relaxed);

        if self.command_ready {
            self.command_ready = false;
            let mut final_cmd = [0u8; 64];
            final_cmd.copy_from_slice(&self.buffer);
            return Some(final_cmd);
        }

        None
    }

    fn process_byte(&mut self, byte: u8, atn: bool) {
        log::info!("[Snooper] Decoded: 0x{:02X} (ATN:{})", byte, atn);
        let listen_addr = self.config.listen_addr.load(Ordering::Relaxed);
        let talk_addr = self.config.talk_addr.load(Ordering::Relaxed);

        if atn {
            if byte == listen_addr {
                // LISTEN device
                self.device_addressed = true;
                self.buffer_len = 0;
            } else if byte == talk_addr {
                // TALK device
                self.device_addressed = true;
                self.buffer_len = 0;
            } else if byte == 0x3F {
                // UNLISTEN
                if self.device_addressed && self.channel_15_active && self.buffer_len > 0 {
                    self.command_ready = true;
                    if self.was_hijacked {
                        self.config.discard_request.store(true, Ordering::Relaxed);
                    }
                }
                self.device_addressed = false;
                self.channel_15_active = false;
                self.config.hijack_active.store(false, Ordering::Relaxed);
                self.was_hijacked = false;
            } else if (byte & 0xF0) == 0x60 && self.device_addressed {
                // SEC
                let channel = byte & 0x0F;
                self.channel_15_active = channel == 15;
            } else if (byte & 0xF0) == 0xF0 && self.device_addressed {
                // OPEN
                let channel = byte & 0x0F;
                self.channel_15_active = channel == 15;
            }
        } else if self.device_addressed && self.channel_15_active {
            if self.buffer_len < 64 {
                self.buffer[self.buffer_len] = byte;
                self.buffer_len += 1;

                // Detect hijack prefixes
                if self.buffer_len == 3 {
                    let prefix = &self.buffer[0..3];
                    if prefix == b"CD:" || prefix == b"MD:" || prefix == b"RD:" {
                        log::info!(
                            "[Snooper] HIJACK ACTIVATED for command: {:?}",
                            core::str::from_utf8(prefix).unwrap_or("???")
                        );
                        self.config.hijack_active.store(true, Ordering::Relaxed);
                        self.was_hijacked = true;
                    }
                }
            }
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer_len
    }

    pub fn should_drive_data(&self) -> bool {
        let hijack_active = self.config.hijack_active.load(Ordering::Relaxed);
        if !hijack_active {
            return false;
        }
        match self.state {
            SnooperState::Ack => true,
            _ => false,
        }
    }
}
