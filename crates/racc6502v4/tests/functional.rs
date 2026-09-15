use racc6502v4::bus::Bus;
use racc6502v4::cpu::CPU;
use std::fs;

struct FunctionalBus {
    memory: Vec<u8>,
}

impl FunctionalBus {
    fn new(bin_path: &str) -> Self {
        let mut memory = fs::read(bin_path).expect("Failed to read functional test binary");
        if memory.len() < 65536 {
            memory.resize(65536, 0);
        }
        Self { memory }
    }
}

impl Bus for FunctionalBus {
    type Ctx = ();
    fn read(&mut self, _ctx: &mut (), addr: u16, _cycle: u32) -> u8 {
        self.memory[addr as usize]
    }

    fn write(&mut self, _ctx: &mut (), addr: u16, value: u8, _cycle: u32) {
        self.memory[addr as usize] = value;
    }

    fn get_instruction_ptr(&self, addr: u16) -> *const u8 {
        &self.memory[addr as usize]
    }
}

#[test]
#[ignore] // This test is very long, run with --ignored
fn test_6502_functional() {
    let mut bus = FunctionalBus::new("tests/bin/6502_functional_test.bin");
    let mut cpu = CPU::new();

    // The test starts at $0400
    cpu.pc = 0x0400;

    let mut last_pc = cpu.pc;
    let mut cycles = 0;
    let max_cycles = 150_000_000;

    println!("Starting 6502 Functional Test at $0400");

    while cycles < max_cycles {
        cycles += cpu.step(&mut bus, &mut ()) as u32;

        if cpu.pc == last_pc {
            // Potential trap
            if cpu.pc == 0x3469 {
                println!(
                    "SUCCESS: Reached success trap at $3469 in {} cycles",
                    cycles
                );
                return;
            } else {
                panic!(
                    "FAILURE: Trapped at ${:04X} after {} cycles",
                    cpu.pc, cycles
                );
            }
        }
        last_pc = cpu.pc;
    }

    panic!(
        "FAILURE: Timeout after {} cycles. Last PC was ${:04X}",
        cycles, cpu.pc
    );
}
