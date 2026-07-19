pub trait CatchUpContext {
    fn catch_up(&mut self, target_cycle: u64);
    fn advance_to(&mut self, target_cycle: u64);
}

pub trait ViaHardware {
    fn read_pa(&mut self, _ctx: &mut impl CatchUpContext) -> u8 { 0xFF }
    fn read_pb(&mut self, _ctx: &mut impl CatchUpContext) -> u8 { 0xFF }
    fn write_pa(&mut self, _ctx: &mut impl CatchUpContext, _val: u8) {}
    fn write_pb(&mut self, _ctx: &mut impl CatchUpContext, _val: u8) {}
    fn write_ddra(&mut self, _ctx: &mut impl CatchUpContext, _val: u8) {}
    fn write_ddrb(&mut self, _ctx: &mut impl CatchUpContext, _val: u8) {}
    fn set_ca1(&mut self, _ctx: &mut impl CatchUpContext, _active: bool) {}
}
