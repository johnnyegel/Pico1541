pub struct EmulatorTime {
    pub last_sync_cycle: u32,
    pub last_byte_cycle: u32,
}

impl EmulatorTime {
    pub fn new() -> Self {
        Self {
            last_sync_cycle: 0,
            last_byte_cycle: 0,
        }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn sync_realtime<H: pico1541_common::interface::HostInterface>(&self, current_cycle: u32, hw: &mut H) {
        hw.spin_until(current_cycle);
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    pub fn consume_catchup_delta(&mut self, target_cycle: u32) -> u32 {
        let delta = target_cycle.wrapping_sub(self.last_sync_cycle);
        // Only consume if target_cycle is strictly ahead or equal to last_sync_cycle
        if delta < 0x80000000 {
            self.last_sync_cycle = target_cycle;
            delta
        } else {
            0
        }
    }
}
