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
        unsafe {
            let prev_idx = if self.index == 0 { 511 } else { self.index - 1 };
            let prev = core::ptr::read_volatile(self.buffer.as_ptr().add(prev_idx));
            
            if prev.via1_pb != record.via1_pb || prev.via2_pb != record.via2_pb || prev.pcr1 != record.pcr1 || prev.pcr2 != record.pcr2 || record.event_type != 0 {
                core::ptr::write_volatile(self.buffer.as_mut_ptr().add(self.index), record);
                core::ptr::write_volatile(&mut self.index, (self.index + 1) % 512);
            }
        }
    }
}

pub static mut FLIGHT_RECORDER: core::mem::MaybeUninit<FlightRecorder> = core::mem::MaybeUninit::uninit();
