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
    device_addressed: bool,
    channel_15_active: bool,
    
    // Layer 3: Command Buffer
    buffer: [u8; 64],
    buffer_len: usize,
    
    pub command_ready: bool,
}

impl IecSnooper {
    pub fn new() -> Self {
        Self {
            state: SnooperState::Idle,
            last_clk: false,
            last_atn: false,
            last_data: false,
            bit_count: 0,
            current_byte: 0,
            atn_at_start: false,
            device_addressed: false,
            channel_15_active: false,
            buffer: [0; 64],
            buffer_len: 0,
            command_ready: false,
        }
    }

    pub fn tick(&mut self, atn: bool, clk: bool, data: bool) -> Option<[u8; 64]> {
        let atn_edge_pos = atn && !self.last_atn;
        let clk_edge_pos = clk && !self.last_clk;
        let clk_edge_neg = !clk && self.last_clk;
        let data_edge_pos = data && !self.last_data;
        let data_edge_neg = !data && self.last_data;

        // If ATN is pulled while we are waiting for a byte, mark it as an ATN byte.
        if atn_edge_pos && self.state == SnooperState::Ready {
            self.atn_at_start = true;
        }

        // --- Layer 1: Byte Decoder ---
        for _ in 0..2 {
            let prev_state = self.state;
            match self.state {
                SnooperState::Idle => {
                    if data_edge_neg {
                        self.atn_at_start = atn;
                        self.state = SnooperState::Ready;
                    }
                }
                SnooperState::Ready => {
                    if clk_edge_pos {
                        self.state = SnooperState::Receiving;
                        self.bit_count = 0;
                        self.current_byte = 0;
                    } else if data_edge_pos && !clk {
                        self.state = SnooperState::Idle;
                    }
                }
                SnooperState::Receiving => {
                    if clk_edge_neg {
                        let bit = if !data { 1 } else { 0 };
                        self.current_byte |= bit << self.bit_count;
                        self.bit_count += 1;
                        
                        if self.bit_count == 8 {
                            self.state = SnooperState::Ack;
                        }
                    }
                }
                SnooperState::Ack => {
                    if data_edge_neg {
                        log::info!("[Snooper] Decoded: 0x{:02X} (ATN:{})", self.current_byte, self.atn_at_start);
                        self.process_byte(self.current_byte, self.atn_at_start);
                        self.state = SnooperState::Idle;
                    }
                }
            }
            if self.state == prev_state { break; }
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
        if atn {
            if byte == 0x28 { // LISTEN 8
                self.device_addressed = true;
                self.buffer_len = 0;
            } else if byte == 0x3F { // UNLISTEN
                if self.device_addressed && self.channel_15_active && self.buffer_len > 0 {
                    self.command_ready = true;
                }
                self.device_addressed = false;
                self.channel_15_active = false;
            } else if (byte & 0xF0) == 0x60 && self.device_addressed { // SEC
                let channel = byte & 0x0F;
                self.channel_15_active = channel == 15;
            } else if (byte & 0xF0) == 0xF0 && self.device_addressed { // OPEN
                let channel = byte & 0x0F;
                self.channel_15_active = channel == 15;
            }
        } else if self.device_addressed && self.channel_15_active {
            if self.buffer_len < 64 {
                self.buffer[self.buffer_len] = byte;
                self.buffer_len += 1;
            }
        }
    }
    
    pub fn buffer_len(&self) -> usize {
        self.buffer_len
    }
}
