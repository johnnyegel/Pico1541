use crate::cpu::CPU;
use crate::bus::Bus;

pub type OpcodeFn<B> = fn(&mut CPU, &mut B, &mut <B as Bus>::Ctx) -> u32;

// We will implement dummy operations for now.
pub fn op_unimpl<B: Bus>(_cpu: &mut CPU, _bus: &mut B, _ctx: &mut B::Ctx) -> u32 {
    2
}
