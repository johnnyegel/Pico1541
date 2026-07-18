pub trait CatchUpContext {
    fn catch_up(&mut self, target_cycle: u64);
    fn advance_to(&mut self, target_cycle: u64);
}

pub trait ViaHardware {
    fn read_pa(&mut self, ctx: &mut impl CatchUpContext) -> u8;
    fn read_pb(&mut self, ctx: &mut impl CatchUpContext) -> u8;
    fn write_pa(&mut self, ctx: &mut impl CatchUpContext, val: u8);
    fn write_pb(&mut self, ctx: &mut impl CatchUpContext, val: u8);
    fn set_ca1(&mut self, ctx: &mut impl CatchUpContext, active: bool);
}
