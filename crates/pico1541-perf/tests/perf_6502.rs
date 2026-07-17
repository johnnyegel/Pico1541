#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_rp as _;

use cortex_m::peripheral::SYST;

pub fn init_cycle_counter(syst: &mut SYST) {
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(0x00FFFFFF);
    syst.clear_current();
    syst.enable_counter();
}

macro_rules! measure_cycles {
    ($code:expr) => {{
        let start = unsafe { (*cortex_m::peripheral::SYST::PTR).cvr.read() };
        $code;
        let end = unsafe { (*cortex_m::peripheral::SYST::PTR).cvr.read() };
        (start.wrapping_sub(end)) & 0x00FFFFFF
    }};
}

#[defmt_test::tests]
mod tests {
    use super::*;


    // A simple loop that will execute continuously
    // 0800: CA        DEX
    // 0801: D0 FD     BNE 0800
    // 0803: 88        DEY
    // 0804: D0 FA     BNE 0800
    // 0806: 4C 06 08  JMP 0806
    const ROM: [u8; 9] = [
        0xCA, 0xD0, 0xFD, 0x88, 0xD0, 0xFA, 0x4C, 0x06, 0x08
    ];

    #[init]
    fn init() -> () {
        let mut cp = cortex_m::Peripherals::take().unwrap();
        init_cycle_counter(&mut cp.SYST);
        ()
    }


    // Bus for mos6502 is provided by mos6502::memory::Memory
    #[test]
    fn test_mos6502() {
        use mos6502::memory::{Bus, Memory};
        use mos6502::cpu::CPU;
        use mos6502::instruction::Nmos6502;
        
        let mut mem = Memory::new();
        mem.set_bytes(0x0800, &ROM);
        mem.set_bytes(0xFFFC, &[0x00, 0x08]); // Reset vector
        
        let mut cpu = CPU::new(mem, Nmos6502);
        cpu.reset();
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                if let Some(instr) = cpu.fetch_next_and_decode() {
                    cpu.execute_instruction(instr);
                }
            }
        });
        
        defmt::info!("mos6502 1000 instructions took {} CPU cycles", cycles);
    }

    // racc6502
    struct RaccBus { memory: [u8; 65536] }
    impl racc6502::bus::Bus for RaccBus {
        type Ctx = ();
        fn read(&mut self, addr: u16, _ctx: &mut ()) -> u8 { self.memory[addr as usize] }
        fn write(&mut self, addr: u16, val: u8, _ctx: &mut ()) { self.memory[addr as usize] = val; }
    }

    #[test]
    fn test_racc6502() {
        use racc6502::cpu::CPU;
        let mut bus = RaccBus { memory: [0; 65536] };
        bus.memory[0x0800..0x0800+ROM.len()].copy_from_slice(&ROM);
        bus.memory[0xFFFC] = 0x00;
        bus.memory[0xFFFD] = 0x08;
        
        let mut cpu = CPU::new();
        cpu.reset();
        
        let cycles = measure_cycles!({
            // tick() does 1 CPU cycle. 1000 instructions is approx 3000 cycles.
            // But let's step exactly 3000 ticks.
            for _ in 0..3000 {
                cpu.tick(&mut bus, &mut ());
            }
        });
        
        defmt::info!("racc6502 3000 ticks took {} CPU cycles", cycles);
    }

    // racc6502v2
    struct Racc2Bus { memory: [u8; 65536] }
    impl racc6502v2::bus::Bus for Racc2Bus {
        type Ctx = ();
        fn read(&mut self, addr: u16, _ctx: &mut ()) -> u8 { self.memory[addr as usize] }
        fn write(&mut self, addr: u16, val: u8, _ctx: &mut ()) { self.memory[addr as usize] = val; }
    }

    #[test]
    fn test_racc6502v2() {
        use racc6502v2::cpu::CPU;
        let mut bus = Racc2Bus { memory: [0; 65536] };
        bus.memory[0x0800..0x0800+ROM.len()].copy_from_slice(&ROM);
        bus.memory[0xFFFC] = 0x00;
        bus.memory[0xFFFD] = 0x08;
        
        let mut cpu = CPU::new();
        cpu.reset();
        
        let cycles = measure_cycles!({
            for _ in 0..3000 {
                cpu.tick(&mut bus, &mut ());
            }
        });
        
        defmt::info!("racc6502v2 3000 ticks took {} CPU cycles", cycles);
    }

    // racc6502v3
    struct Racc3Bus { memory: [u8; 65536] }
    impl racc6502v3::bus::Bus for Racc3Bus {
        type Ctx = ();
        #[unsafe(link_section = ".data.ram_func")]
        #[inline(always)]
        fn read(&mut self, addr: u16, _cycle: u32, _ctx: &mut ()) -> u8 { self.memory[addr as usize] }

        #[unsafe(link_section = ".data.ram_func")]
        #[inline(always)]
        fn write(&mut self, addr: u16, val: u8, _cycle: u32, _ctx: &mut ()) { self.memory[addr as usize] = val; }
    }

    #[test]
    fn test_racc6502v3() {
        use racc6502v3::cpu::CPU;
        let mut bus = Racc3Bus { memory: [0; 65536] };
        bus.memory[0x0800..0x0800+ROM.len()].copy_from_slice(&ROM);
        bus.memory[0xFFFC] = 0x00;
        bus.memory[0xFFFD] = 0x08;
        
        let mut cpu = CPU::new();
        cpu.reset(&mut bus, &mut ());
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                cpu.step(&mut bus, &mut ());
            }
        });
        
        defmt::info!("racc6502v3 1000 instructions took {} CPU cycles", cycles);
    }

    // racc6502v4
    pub struct Racc4Bus { memory: [u8; 65536] }
    impl racc6502v4::bus::Bus for Racc4Bus {
        type Ctx = ();
        fn read(&mut self, addr: u16, _cycle: u32, _ctx: &mut ()) -> u8 { self.memory[addr as usize] }
        fn get_instruction_ptr(&self, addr: u16) -> *const u8 { unsafe { self.memory.as_ptr().add(addr as usize) } }
        fn write(&mut self, addr: u16, val: u8, _cycle: u32, _ctx: &mut ()) { self.memory[addr as usize] = val; }
    }

    #[test]
    fn test_racc6502v4() {
        use racc6502v4::cpu::CPU;
        let mut bus = Racc4Bus { memory: [0; 65536] };
        bus.memory[0x0800..0x0800+ROM.len()].copy_from_slice(&ROM);
        bus.memory[0xFFFC] = 0x00;
        bus.memory[0xFFFD] = 0x08;
        
        let mut cpu = CPU::new();
        cpu.reset(&mut bus, &mut ());
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                cpu.step(&mut bus, &mut ());
            }
        });
        
        defmt::info!("racc6502v4 1000 instructions took {} CPU cycles", cycles);
    }
}

// Added to easily inspect the generated assembly for the step function.
#[no_mangle]
#[inline(never)]
pub fn inspect_racc6502v4_step(cpu: &mut racc6502v4::cpu::CPU, bus: &mut tests::Racc4Bus) {
    cpu.step(bus, &mut ());
}

