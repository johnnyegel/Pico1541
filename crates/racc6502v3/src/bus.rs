pub trait Bus {
    type Ctx;
    fn read(&mut self, addr: u16, ctx: &mut Self::Ctx) -> u8;
    fn write(&mut self, addr: u16, value: u8, ctx: &mut Self::Ctx);

    fn irq_pending(&mut self) -> bool { false }
    fn nmi_pending(&mut self) -> bool { false }
    fn so_pending(&mut self) -> bool { false }
}
