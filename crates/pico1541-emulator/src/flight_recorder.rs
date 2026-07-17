#[repr(C)]
#[derive(Clone, Copy)]
pub struct FlightRecord {
    pub cycle: u32,
    pub pc: u32,
    pub event_type: u8,
    pub via1_pb: u8,
    pub via2_pb: u8,
    pub pcr1: u8,
    pub pcr2: u8,
    pub via2_pa: u8,
    pub is_sync: u8,
    pub _padding: u8,
}

pub struct FlightRecorder {
    pub buffer: [FlightRecord; 512],
    pub index: usize,
    pub last_record: FlightRecord,
}

impl FlightRecorder {
    pub fn record(&mut self, record: FlightRecord) {
        let prev = &self.last_record;
        
        if prev.via1_pb != record.via1_pb || prev.via2_pb != record.via2_pb || prev.pcr1 != record.pcr1 || prev.pcr2 != record.pcr2 || prev.pc != record.pc || record.event_type != 0 || prev.via2_pa != record.via2_pa || prev.is_sync != record.is_sync {
            unsafe {
                core::ptr::write_volatile(self.buffer.as_mut_ptr().add(self.index), record);
                core::ptr::write_volatile(&mut self.index, (self.index + 1) % 512);
            }
            self.last_record = record;
        }
    }
}

pub static mut FLIGHT_RECORDER: core::mem::MaybeUninit<FlightRecorder> = core::mem::MaybeUninit::zeroed();

pub static mut RAM_POINTER: *const u8 = core::ptr::null();
