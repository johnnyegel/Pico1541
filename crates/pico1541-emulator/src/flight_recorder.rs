#[derive(Clone, Copy)]
pub struct FlightRecord {
    pub cycle: u32,
    pub event_type: u8,
    pub via1_pb: u8,
    pub via2_pb: u8,
    pub pcr1: u8,
    pub pcr2: u8,
}

pub struct FlightRecorder {
    pub buffer: [FlightRecord; 512],
    pub index: usize,
}

impl FlightRecorder {
    pub fn record(&mut self, record: FlightRecord) {
        self.buffer[self.index] = record;
        self.index = (self.index + 1) % 512;
    }
}

pub static mut FLIGHT_RECORDER: core::mem::MaybeUninit<FlightRecorder> = core::mem::MaybeUninit::uninit();
