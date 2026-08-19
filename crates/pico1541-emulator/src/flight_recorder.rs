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

#[repr(C)]
pub struct FlightRecorder {
    pub buffer: [FlightRecord; 512],
    pub index: usize,
    pub last_record: FlightRecord,
}

impl FlightRecorder {
    #[inline(always)]
    #[link_section = ".data.ram_func"]
    pub fn record(&mut self, record: FlightRecord) {
        unsafe {
            core::ptr::write_volatile(self.buffer.as_mut_ptr().add(self.index), record);
            core::ptr::write_volatile(&mut self.index, (self.index + 1) % 512);
        }
        self.last_record = record;
    }
}

pub static mut FLIGHT_RECORDER: core::mem::MaybeUninit<FlightRecorder> = core::mem::MaybeUninit::zeroed();
