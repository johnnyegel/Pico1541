use crate::bus::Bus;
use crate::cpu::{CPU, FLAG_B, FLAG_C, FLAG_D, FLAG_I, FLAG_N, FLAG_U, FLAG_V, FLAG_Z};

pub type OpcodeFn<B> = fn(&mut CPU, &mut B, &mut <B as Bus>::Ctx) -> u32;

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_00<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cpu.pc = cpu.pc.wrapping_add(1);
    {
        cpu.push_word(bus, ctx, offset, cpu.pc);
        offset += 2;
    };
    {
        cpu.push(bus, ctx, offset, cpu.p | FLAG_B | FLAG_U);
        offset += 1;
    };
    cpu.p |= FLAG_I;
    cpu.pc = {
        let v = cpu.read_word(bus, ctx, offset, 0xFFFE);
        offset += 2;
        v
    };
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_01<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_02<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_03<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_04<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_05<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_06<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val <<= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_07<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_08<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    {
        cpu.push(bus, ctx, offset, cpu.p | FLAG_B | FLAG_U);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_09<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    if cpu.a & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.a <<= 1;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val <<= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_0f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_10<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_N == 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_11<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_12<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_13<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_14<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_15<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_16<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val <<= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_17<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_19<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val <<= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_1f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a |= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_20<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    {
        cpu.push_word(bus, ctx, offset, cpu.pc.wrapping_sub(1));
        offset += 2;
    };
    cpu.pc = addr;
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_21<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_22<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_23<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_24<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let res = cpu.a & val;
    cpu.set_z(res);
    cpu.p = (cpu.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_25<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_26<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val << 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_27<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_28<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cpu.p = {
        let v = cpu.pop(bus, ctx, offset);
        offset += 1;
        v
    } | FLAG_U;
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_29<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
    if cpu.a & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.a = (cpu.a << 1) | c;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let res = cpu.a & val;
    cpu.set_z(res);
    cpu.p = (cpu.p & !(FLAG_N | FLAG_V)) | (val & (FLAG_N | FLAG_V));
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val << 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_2f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_30<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_N != 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_31<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_32<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_33<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_34<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_35<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_36<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val << 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_37<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_39<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 1 } else { 0 };
    if val & 0x80 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val << 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_3f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a &= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_40<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cpu.p = {
        let v = cpu.pop(bus, ctx, offset);
        offset += 1;
        v
    } | FLAG_U;
    cpu.pc = {
        let v = cpu.pop_word(bus, ctx, offset);
        offset += 2;
        v
    };
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_41<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_42<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_43<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_44<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_45<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_46<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val >>= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_47<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_48<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    {
        cpu.push(bus, ctx, offset, cpu.a);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_49<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    if cpu.a & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.a >>= 1;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    cpu.pc = addr;
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val >>= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_4f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_50<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_V == 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_51<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_52<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_53<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_54<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_55<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_56<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val >>= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_57<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_59<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val >>= 1;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_5f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a ^= val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_60<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cpu.pc = {
        let v = cpu.pop_word(bus, ctx, offset);
        offset += 2;
        v
    }
    .wrapping_add(1);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_61<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_62<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_63<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_64<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_65<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_66<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val >> 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_67<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_68<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cpu.a = {
        let v = cpu.pop(bus, ctx, offset);
        offset += 1;
        v
    };
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_69<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.do_adc(val);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
    if cpu.a & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.a = (cpu.a >> 1) | c;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        if ptr & 0x00FF == 0x00FF {
            ({
                let v = cpu.read(bus, ctx, offset, ptr & 0xFF00);
                offset += 1;
                v
            } as u16)
                << 8
                | {
                    let v = cpu.read(bus, ctx, offset, ptr);
                    offset += 1;
                    v
                } as u16
        } else {
            {
                let v = cpu.read_word(bus, ctx, offset, ptr);
                offset += 2;
                v
            }
        }
    };
    cpu.pc = addr;
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val >> 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_6f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_70<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_V != 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_71<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_72<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_73<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_74<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_75<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_76<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val >> 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_77<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_79<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7a<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let c = if cpu.p & FLAG_C != 0 { 0x80 } else { 0 };
    if val & 0x01 != 0 {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    val = (val >> 1) | c;
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_7f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_adc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_80<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_81<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_82<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_83<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a & cpu.x);
        offset += 1;
    };
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_84<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.y);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_85<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_86<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.x);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_87<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.a & cpu.x);
        offset += 1;
    };
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_89<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_8b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_8c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.y);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_8d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_8e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.x);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_8f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a & cpu.x);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_90<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_C == 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_91<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_92<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_93<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_94<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.y);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_95<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_96<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.y) as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.x);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_97<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    {
        cpu.write(bus, ctx, offset, addr, cpu.a & cpu.x);
        offset += 1;
    };
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_99<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_9b<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_9c<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_9d<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    {
        cpu.write(bus, ctx, offset, addr, cpu.a);
        offset += 1;
    };
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_9e<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_9f<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.y = val;
    cpu.set_zn(cpu.y);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.x = val;
    cpu.set_zn(cpu.x);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.y = val;
    cpu.set_zn(cpu.y);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.x = val;
    cpu.set_zn(cpu.x);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_a9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ab<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ac<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.y = val;
    cpu.set_zn(cpu.y);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ad<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ae<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.x = val;
    cpu.set_zn(cpu.x);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_af<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_C != 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.y = val;
    cpu.set_zn(cpu.y);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.y) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.x = val;
    cpu.set_zn(cpu.x);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_b9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_bb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_bc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.y = val;
    cpu.set_zn(cpu.y);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_bd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_be<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.x = val;
    cpu.set_zn(cpu.x);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_bf<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.a = val;
    cpu.x = val;
    cpu.set_zn(cpu.a);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    let diff = (cpu.y as u16).wrapping_sub(val as u16);
    if cpu.y >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.y as u16).wrapping_sub(val as u16);
    if cpu.y >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_sub(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_c9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_cb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_cc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.y as u16).wrapping_sub(val as u16);
    if cpu.y >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_cd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ce<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_sub(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_cf<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_Z == 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_sub(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_d9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_da<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_db<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_dc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_dd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_de<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_sub(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_df<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.a as u16).wrapping_sub(val as u16);
    if cpu.a >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    let diff = (cpu.x as u16).wrapping_sub(val as u16);
    if cpu.x >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        }
        .wrapping_add(cpu.x);
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        (hi << 8) | lo
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.x as u16).wrapping_sub(val as u16);
    if cpu.x >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_add(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 5;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 3;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_e9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let val = addr as u8;
    cpu.do_sbc(val);
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ea<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = 0;
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_eb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ec<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    let diff = (cpu.x as u16).wrapping_sub(val as u16);
    if cpu.x >= val {
        cpu.p |= FLAG_C;
    } else {
        cpu.p &= !FLAG_C;
    }
    cpu.set_zn(diff as u8);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ed<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ee<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_add(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ef<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_word_pc(bus, ctx);
        offset += 2;
        v
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f0<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    } as u16;
    let rel = addr as i8;
    if cpu.p & FLAG_Z != 0 {
        let new_pc = cpu.pc.wrapping_add(rel as u16);
        cycles += if (new_pc & 0xFF00) != (cpu.pc & 0xFF00) {
            2
        } else {
            1
        };
        cpu.pc = new_pc;
    }
    cycles += 2;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f1<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f2<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f3<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let ptr = {
            let v = cpu.read_pc(bus, ctx);
            offset += 1;
            v
        };
        let lo = {
            let v = cpu.read(bus, ctx, offset, ptr as u16);
            offset += 1;
            v
        } as u16;
        let hi = {
            let v = cpu.read(bus, ctx, offset, ptr.wrapping_add(1) as u16);
            offset += 1;
            v
        } as u16;
        let base = (hi << 8) | lo;
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 5;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f4<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f5<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f6<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_add(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 6;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f7<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    }
    .wrapping_add(cpu.x) as u16;
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
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
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_f9<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_fa<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_fb<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.y as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_fc<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    {
        let v = cpu.read_pc(bus, ctx);
        offset += 1;
        v
    };
    2
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_fd<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_fe<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let mut val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    val = val.wrapping_add(1);
    {
        cpu.write(bus, ctx, offset, addr, val);
        offset += 1;
    };
    cpu.set_zn(val);
    cycles += 7;
    cycles
}

#[allow(unused_variables, unused_assignments, unused_mut)]
#[link_section = ".core1_text"]
#[inline(always)]
pub fn op_ff<B: Bus>(cpu: &mut CPU, bus: &mut B, ctx: &mut <B as Bus>::Ctx) -> u32 {
    let mut cycles: u32 = 0;
    let mut offset: u32 = 1;
    let mut page_cross = false;

    let addr = {
        let base = {
            let v = cpu.read_word_pc(bus, ctx);
            offset += 2;
            v
        };
        let a = base.wrapping_add(cpu.x as u16);
        page_cross = (a & 0xFF00) != (base & 0xFF00);
        a
    };
    let val = {
        let v = cpu.read(bus, ctx, offset, addr);
        offset += 1;
        v
    };
    cpu.do_sbc(val);
    cycles += 4;
    if page_cross {
        cycles += 1;
    }
    cycles
}
