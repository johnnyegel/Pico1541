#![no_std]

pub mod cpu;
pub mod bus;
pub mod opcodes;

#[cfg(test)]
mod cycle_tests {
    use super::*;
    
    struct MyMem {
        mem: [u8; 65536],
    }
    impl Memory for MyMem {
        fn read(&mut self, addr: u16) -> u8 {
            self.mem[addr as usize]
        }
        fn write(&mut self, addr: u16, val: u8) {
            self.mem[addr as usize] = val;
        }
    }

    #[test]
    fn test_loop_cycles() {
        let mut mem = MyMem { mem: [0; 65536] };
        mem.mem[0x03CC] = 0xAE;
        mem.mem[0x03CD] = 0x00;
        mem.mem[0x03CE] = 0x18;
        mem.mem[0x03CF] = 0xD0;
        mem.mem[0x03D0] = 0xFB;
        
        let mut cpu = CPU::new();
        cpu.pc = 0x03CC;
        cpu.z = false;
        
        cpu.step(&mut mem);
        let c1 = cpu.cycles;
        cpu.step(&mut mem);
        let c2 = cpu.cycles;
        
        println!("LDX cycles: {}", c1);
        println!("BNE cycles: {}", c2 - c1);
        println!("Total loop: {}", c2);
        panic!("Show output");
    }
}
