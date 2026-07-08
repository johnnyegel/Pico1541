use crate::cpu::{CPU, FLAG_B, FLAG_C, FLAG_D, FLAG_I, FLAG_N, FLAG_U, FLAG_V, FLAG_Z};
use crate::bus::Bus;

pub type OpcodeFn<B> = fn(&mut CPU, &mut B, &mut <B as Bus>::Ctx) -> u32;

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_00<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.pc = cpu.pc.wrapping_add(1);
                cpu.push_word(bus, ctx, &mut offset, cpu.pc);
                cpu.push(bus, ctx, &mut offset, cpu.p | FLAG_B | FLAG_U);
                cpu.p |= FLAG_I;
                cpu.pc = cpu.read_word(bus, ctx, &mut offset, 0xFFFE);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_01<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_02<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_03<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_04<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_05<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_06<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val <<= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_07<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_08<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.push(bus, ctx, &mut offset, cpu.p | FLAG_B | FLAG_U);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_09<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                if cpu.a & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.a <<= 1;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val <<= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_0f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_10<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_N == 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_11<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_12<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_13<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_14<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_15<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_16<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val <<= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_17<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_18<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p &= !FLAG_C;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_19<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a |= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val <<= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_1f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_20<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                cpu.push_word(bus, ctx, &mut offset, cpu.pc.wrapping_sub(1));
                cpu.pc = addr;
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_21<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_22<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_23<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_24<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let res = cpu.a & val;
                cpu.set_z(res);
                cpu.p = (cpu.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_25<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_26<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val << 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_27<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_28<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p = cpu.pop(bus, ctx, &mut offset) | FLAG_U;
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_29<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
                if cpu.a & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.a = (cpu.a << 1) | c;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let res = cpu.a & val;
                cpu.set_z(res);
                cpu.p = (cpu.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val << 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_2f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_30<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_N != 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_31<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_32<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_33<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_34<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_35<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_36<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val << 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_37<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_38<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p |= FLAG_C;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_39<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a &= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
                if val & 0x80 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val << 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_3f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_40<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p = cpu.pop(bus, ctx, &mut offset) | FLAG_U;
                cpu.pc = cpu.pop_word(bus, ctx, &mut offset);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_41<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_42<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_43<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_44<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_45<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_46<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val >>= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_47<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_48<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.push(bus, ctx, &mut offset, cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_49<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                if cpu.a & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.a >>= 1;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                cpu.pc = addr;
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val >>= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_4f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_50<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_V == 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_51<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_52<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_53<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_54<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_55<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_56<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val >>= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_57<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_58<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p &= !FLAG_I;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_59<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a ^= val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val >>= 1;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_5f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_60<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.pc = cpu.pop_word(bus, ctx, &mut offset).wrapping_add(1);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_61<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_62<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_63<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_64<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_65<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_66<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val >> 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_67<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_68<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.a = cpu.pop(bus, ctx, &mut offset);
                cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_69<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.do_adc(val);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
                if cpu.a & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.a = (cpu.a >> 1) | c;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_word_pc(bus, ctx, &mut offset); if ptr & 0x00FF == 0x00FF { (cpu.read(bus, ctx, &mut offset, ptr & 0xFF00) as u16) << 8 | cpu.read(bus, ctx, &mut offset, ptr) as u16 } else { cpu.read_word(bus, ctx, &mut offset, ptr) } };
                cpu.pc = addr;
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val >> 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_6f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_70<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_V != 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_71<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_72<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_73<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_74<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_75<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_76<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val >> 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_77<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_78<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p |= FLAG_I;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_79<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_adc(val);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
                if val & 0x01 != 0 { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                val = (val >> 1) | c;
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_7f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_80<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_81<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_82<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_83<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_84<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.y);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_85<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_86<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.x);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_87<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_88<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.y = cpu.y.wrapping_sub(1);
                cpu.set_zn(cpu.y);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_89<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.a = cpu.x;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                cpu.write(bus, ctx, &mut offset, addr, cpu.y);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                cpu.write(bus, ctx, &mut offset, addr, cpu.x);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_8f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_90<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_C == 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_91<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_92<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_93<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_94<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.y);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_95<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_96<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.y) as u16;
                cpu.write(bus, ctx, &mut offset, addr, cpu.x);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_97<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_98<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.a = cpu.y;
                cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_99<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.s = cpu.x;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                cpu.write(bus, ctx, &mut offset, addr, cpu.a);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_9f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.y = val; cpu.set_zn(cpu.y);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.x = val; cpu.set_zn(cpu.x);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.y = val; cpu.set_zn(cpu.y);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.x = val; cpu.set_zn(cpu.x);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.y = cpu.a;
                cpu.set_zn(cpu.y);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_a9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_aa<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.x = cpu.a;
                cpu.set_zn(cpu.x);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ab<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ac<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.y = val; cpu.set_zn(cpu.y);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ad<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ae<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.x = val; cpu.set_zn(cpu.x);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_af<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_C != 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.y = val; cpu.set_zn(cpu.y);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.y) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.x = val; cpu.set_zn(cpu.x);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p &= !FLAG_V;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_b9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ba<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.x = cpu.s;
                cpu.set_zn(cpu.x);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_bb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_bc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.y = val; cpu.set_zn(cpu.y);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_bd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.a = val; cpu.set_zn(cpu.a);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_be<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.x = val; cpu.set_zn(cpu.x);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_bf<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                let diff = (cpu.y as u16).wrapping_sub(val as u16);
                if cpu.y >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.y as u16).wrapping_sub(val as u16);
                if cpu.y >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_sub(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.y = cpu.y.wrapping_add(1);
                cpu.set_zn(cpu.y);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_c9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ca<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.x = cpu.x.wrapping_sub(1);
                cpu.set_zn(cpu.x);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_cb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_cc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.y as u16).wrapping_sub(val as u16);
                if cpu.y >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_cd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ce<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_sub(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_cf<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_Z == 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_sub(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p &= !FLAG_D;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_d9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_da<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_db<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_dc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_dd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.a as u16).wrapping_sub(val as u16);
                if cpu.a >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_de<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_sub(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_df<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                let diff = (cpu.x as u16).wrapping_sub(val as u16);
                if cpu.x >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; (hi << 8) | lo };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.x as u16).wrapping_sub(val as u16);
                if cpu.x >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 3;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_add(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 5;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.x = cpu.x.wrapping_add(1);
                cpu.set_zn(cpu.x);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_e9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let val = addr as u8;
                cpu.do_sbc(val);
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ea<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_eb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ec<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                let diff = (cpu.x as u16).wrapping_sub(val as u16);
                if cpu.x >= val { cpu.p |= FLAG_C; } else { cpu.p &= !FLAG_C; }
                cpu.set_zn(diff as u8);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ed<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ee<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_word_pc(bus, ctx, &mut offset);
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_add(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ef<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset) as u16;
                let rel = addr as i8;
                if cpu.p & FLAG_Z != 0 {
                    let new_pc = cpu.pc.wrapping_add(rel as u16);
                    cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) { 2 } else { 1 };
                    cpu.pc = new_pc;
                }
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let ptr = cpu.read_pc(bus, ctx, &mut offset); let lo = cpu.read(bus, ctx, &mut offset, ptr as u16) as u16; let hi = cpu.read(bus, ctx, &mut offset, ptr.wrapping_add(1) as u16) as u16; let base = (hi << 8) | lo; let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 5;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 4;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = cpu.read_pc(bus, ctx, &mut offset).wrapping_add(cpu.x) as u16;
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_add(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 6;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f8<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = 0;
                cpu.p |= FLAG_D;
                cycles += 2;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_f9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.y as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_fa<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_fb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_fc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_fd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let val = cpu.read(bus, ctx, &mut offset, addr);
                cpu.do_sbc(val);
                cycles += 4;
                if page_cross { cycles += 1; }
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_fe<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

                let addr = { let base = cpu.read_word_pc(bus, ctx, &mut offset); let a = base.wrapping_add(cpu.x as u16); page_cross = (a & 0xFF00) != (base & 0xFF00); a };
                let mut val = cpu.read(bus, ctx, &mut offset, addr);
                val = val.wrapping_add(1);
                cpu.write(bus, ctx, &mut offset, addr, val);
                cpu.set_zn(val);
                cycles += 7;
                cycles
            }

#[allow(unused_variables, unused_assignments, unused_mut)]
pub fn op_ff<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

 cpu.read_pc(bus, ctx, &mut offset); 2 }

pub const fn get_opcodes<B: Bus>() -> [OpcodeFn<B>; 256] {
    [
        op_00,
        op_01,
        op_02,
        op_03,
        op_04,
        op_05,
        op_06,
        op_07,
        op_08,
        op_09,
        op_0a,
        op_0b,
        op_0c,
        op_0d,
        op_0e,
        op_0f,
        op_10,
        op_11,
        op_12,
        op_13,
        op_14,
        op_15,
        op_16,
        op_17,
        op_18,
        op_19,
        op_1a,
        op_1b,
        op_1c,
        op_1d,
        op_1e,
        op_1f,
        op_20,
        op_21,
        op_22,
        op_23,
        op_24,
        op_25,
        op_26,
        op_27,
        op_28,
        op_29,
        op_2a,
        op_2b,
        op_2c,
        op_2d,
        op_2e,
        op_2f,
        op_30,
        op_31,
        op_32,
        op_33,
        op_34,
        op_35,
        op_36,
        op_37,
        op_38,
        op_39,
        op_3a,
        op_3b,
        op_3c,
        op_3d,
        op_3e,
        op_3f,
        op_40,
        op_41,
        op_42,
        op_43,
        op_44,
        op_45,
        op_46,
        op_47,
        op_48,
        op_49,
        op_4a,
        op_4b,
        op_4c,
        op_4d,
        op_4e,
        op_4f,
        op_50,
        op_51,
        op_52,
        op_53,
        op_54,
        op_55,
        op_56,
        op_57,
        op_58,
        op_59,
        op_5a,
        op_5b,
        op_5c,
        op_5d,
        op_5e,
        op_5f,
        op_60,
        op_61,
        op_62,
        op_63,
        op_64,
        op_65,
        op_66,
        op_67,
        op_68,
        op_69,
        op_6a,
        op_6b,
        op_6c,
        op_6d,
        op_6e,
        op_6f,
        op_70,
        op_71,
        op_72,
        op_73,
        op_74,
        op_75,
        op_76,
        op_77,
        op_78,
        op_79,
        op_7a,
        op_7b,
        op_7c,
        op_7d,
        op_7e,
        op_7f,
        op_80,
        op_81,
        op_82,
        op_83,
        op_84,
        op_85,
        op_86,
        op_87,
        op_88,
        op_89,
        op_8a,
        op_8b,
        op_8c,
        op_8d,
        op_8e,
        op_8f,
        op_90,
        op_91,
        op_92,
        op_93,
        op_94,
        op_95,
        op_96,
        op_97,
        op_98,
        op_99,
        op_9a,
        op_9b,
        op_9c,
        op_9d,
        op_9e,
        op_9f,
        op_a0,
        op_a1,
        op_a2,
        op_a3,
        op_a4,
        op_a5,
        op_a6,
        op_a7,
        op_a8,
        op_a9,
        op_aa,
        op_ab,
        op_ac,
        op_ad,
        op_ae,
        op_af,
        op_b0,
        op_b1,
        op_b2,
        op_b3,
        op_b4,
        op_b5,
        op_b6,
        op_b7,
        op_b8,
        op_b9,
        op_ba,
        op_bb,
        op_bc,
        op_bd,
        op_be,
        op_bf,
        op_c0,
        op_c1,
        op_c2,
        op_c3,
        op_c4,
        op_c5,
        op_c6,
        op_c7,
        op_c8,
        op_c9,
        op_ca,
        op_cb,
        op_cc,
        op_cd,
        op_ce,
        op_cf,
        op_d0,
        op_d1,
        op_d2,
        op_d3,
        op_d4,
        op_d5,
        op_d6,
        op_d7,
        op_d8,
        op_d9,
        op_da,
        op_db,
        op_dc,
        op_dd,
        op_de,
        op_df,
        op_e0,
        op_e1,
        op_e2,
        op_e3,
        op_e4,
        op_e5,
        op_e6,
        op_e7,
        op_e8,
        op_e9,
        op_ea,
        op_eb,
        op_ec,
        op_ed,
        op_ee,
        op_ef,
        op_f0,
        op_f1,
        op_f2,
        op_f3,
        op_f4,
        op_f5,
        op_f6,
        op_f7,
        op_f8,
        op_f9,
        op_fa,
        op_fb,
        op_fc,
        op_fd,
        op_fe,
        op_ff,
    ]
}