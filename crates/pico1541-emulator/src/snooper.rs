#[derive(Debug, Clone, Copy, PartialEq)]
enum SnooperState {
    Idle,
    Ready,
    Receiving,
    Ack,
}

pub struct IecSnooper {
    state: SnooperState,
    last_clk: bool,
    last_atn: bool,
    last_data: bool,

    // Layer 1: Byte Decoder State
    bit_count: u8,
    current_byte: u8,
    atn_at_start: bool,

    // Layer 2: Protocol State
    pub listen_addr: u8,
    pub talk_addr: u8,
    device_addressed: bool,
    channel_15_active: bool,

    // Layer 3: Command Buffer
    buffer: [u8; 64],
    buffer_len: usize,

    pub command_ready: bool,
    pub hijack_active: bool,
    pub discard_request: bool,
    ack_seen: bool,
    ready_delay: u8,
    was_hijacked: bool,
    clk_released: bool,
}

impl IecSnooper {
    pub fn new(device_id: u8) -> Self {
        Self {
            state: SnooperState::Idle,
            last_clk: false,
            last_atn: false,
            last_data: false,
            bit_count: 0,
            current_byte: 0,
            atn_at_start: false,
            listen_addr: 0x20 | device_id,
            talk_addr: 0x40 | device_id,
            device_addressed: false,
            channel_15_active: false,
            buffer: [0; 64],
            buffer_len: 0,
            command_ready: false,
            hijack_active: false,
            discard_request: false,
            ack_seen: false,
            ready_delay: 0,
            was_hijacked: false,
            clk_released: false,
        }
    }

    #[allow(unreachable_code, unused_variables)]
    pub fn tick(&mut self, atn: bool, clk: bool, data: bool) -> Option<[u8; 64]> {
        let atn_edge_pos = atn && !self.last_atn;
        let clk_edge_pos = clk && !self.last_clk;
        let clk_edge_neg = !clk && self.last_clk;
        let data_edge_pos = data && !self.last_data;
        let data_edge_neg = !data && self.last_data;

        // If ATN is pulled while we are waiting for a byte, mark it as an ATN byte.
        if atn_edge_pos {
            self.atn_at_start = true;
        }

        // Snooper is currently disabled during protocol refactor
        return None;

        // --- Layer 1: Byte Decoder ---
        match self.state {
            SnooperState::Idle => {
                // Enter Ready on DATA pull (standard) or CLK/ATN transitions if hijacked (ROM blinded)
                if data_edge_neg || (self.hijack_active && (clk_edge_neg || atn_edge_pos)) {
                    self.atn_at_start = atn;
                    self.state = SnooperState::Ready;
                    self.ready_delay = 20;
                    self.clk_released = false;
                    log::trace!("Idle state: Detected start of byte (DATA edge or hijack trigger), entering Ready state...");
                }
            }
            SnooperState::Ready => {
                if !clk {
                    self.clk_released = true;
                }

                if self.ready_delay > 0 {
                    self.ready_delay -= 1;
                } else {
                    if clk_edge_pos && self.clk_released {
                        self.state = SnooperState::Receiving;
                        self.bit_count = 0;
                        self.current_byte = 0;
                        log::trace!(
                            "Ready state: CLK posedge detected, starting to receive byte..."
                        );
                    } else if data_edge_pos && !clk && !self.should_drive_data() {
                        self.state = SnooperState::Idle;
                    }
                }
            }
            SnooperState::Receiving => {
                if clk_edge_neg {
                    let bit = if !data { 1 } else { 0 };
                    log::trace!(
                        "Receiving state: CLK negedge detected, reading bit {}: {}",
                        self.bit_count,
                        bit
                    );
                    self.current_byte |= bit << self.bit_count;
                    self.bit_count += 1;

                    if self.bit_count == 8 {
                        self.process_byte(self.current_byte, self.atn_at_start);
                        self.state = SnooperState::Ack;
                        log::trace!(
                            "Receiving state: Finished receiving byte, entering Ack state..."
                        );
                    }
                }
            }
            SnooperState::Ack => {
                if clk {
                    self.ack_seen = true;
                }
                if self.ack_seen && !clk {
                    self.state = SnooperState::Idle;
                    self.ack_seen = false;
                }
            }
        }
        self.last_atn = atn;
        self.last_clk = clk;
        self.last_data = data;

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
        if atn {
            if byte == self.listen_addr {
                // LISTEN device
                self.device_addressed = true;
                self.buffer_len = 0;
            } else if byte == self.talk_addr {
                // TALK device
                self.device_addressed = true;
                self.buffer_len = 0;
            } else if byte == 0x3F {
                // UNLISTEN
                if self.device_addressed && self.channel_15_active && self.buffer_len > 0 {
                    self.command_ready = true;
                    if self.was_hijacked {
                        self.discard_request = true;
                    }
                }
                self.device_addressed = false;
                self.channel_15_active = false;
                self.hijack_active = false;
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
                        self.hijack_active = true;
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
        if !self.hijack_active {
            return false;
        }
        match self.state {
            SnooperState::Ready => self.ready_delay == 0 && !self.clk_released,
            SnooperState::Ack => true,
            _ => false,
        }
    }

    pub fn set_iec_addresses(&mut self, listen: u8, talk: u8) {
        // Only update if they look like valid IEC addresses (LISTEN=0x20-0x3E, TALK=0x40-0x5E)
        if (listen & 0xE0) == 0x20 && (talk & 0xE0) == 0x40 {
            log::trace!(
                "[Snooper] Updated IEC addresses: LISTEN=0x{:02X}, TALK=0x{:02X}",
                listen,
                talk
            );
            self.listen_addr = listen;
            self.talk_addr = talk;
        }
    }
}
