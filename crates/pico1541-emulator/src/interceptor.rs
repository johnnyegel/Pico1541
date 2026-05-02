use pico1541_common::snooper::IecBusByte;

/// A lightweight interceptor that samples the IEC bus state every cycle.
/// It only reports changes to avoid flooding the communication channel.
pub struct BusInterceptor {
    last_state: IecBusByte,
    cycles_since_last: u32,
}

impl BusInterceptor {
    pub fn new() -> Self {
        Self {
            last_state: IecBusByte(0),
            cycles_since_last: 0,
        }
    }

    /// Samples the bus lines. Returns Some((IecBusByte, Delta)) if the state has changed.
    pub fn tick(&mut self, state: IecBusByte) -> Option<(IecBusByte, u32)> {
        self.cycles_since_last = self.cycles_since_last.saturating_add(1);
        if state != self.last_state {
            let delta = self.cycles_since_last;
            self.cycles_since_last = 0;
            self.last_state = state;
            Some((state, delta))
        } else {
            None
        }
    }
}
