use crate::bus::Bus;

pub struct CPU {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub s: u8,
    pub pc: u16,
    pub p: u8,
    pub cycles: u32,
}

impl CPU {
    pub fn new() -> Self {
        Self {
            a: 0, x: 0, y: 0, s: 0xFD, pc: 0, p: 0x24, cycles: 0,
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(never)]
    pub fn step<B: Bus>(&mut self, bus: &mut B, ctx: &mut B::Ctx, jump_table: &[crate::opcodes::OpcodeFn<B>; 256]) -> u32 {
        let opcode = bus.read(self.pc, ctx);
        self.pc = self.pc.wrapping_add(1);
        
        // Jump table dispatch
        let elapsed = jump_table[opcode as usize](self, bus, ctx);
        self.cycles = self.cycles.wrapping_add(elapsed);
        elapsed
    }
}
