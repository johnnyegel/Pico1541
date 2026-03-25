use racc6502::cpu::CPU;
use racc6502::bus::Bus;
use racc6502::instructions::nmos6502::INSTRUCTION_TABLE;

struct DebugBus {
    memory: [u8; 65536],
}

impl Bus for DebugBus {
    fn read(&mut self, addr: u16) -> u8 { self.memory[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.memory[addr as usize] = val; }
}

#[test]
#[cfg(debug_assertions)]
fn test_debug_hook_and_mnemonics() {
    let mut bus = DebugBus { memory: [0; 65536] };
    
    // Setup memory:
    // $FFFC: $00 $10 (Reset vector -> $1000)
    // $1000: $A9 $42 (LDA #$42)
    // $1002: $AA     (TAX)
    bus.memory[0xFFFC] = 0x00;
    bus.memory[0xFFFD] = 0x10;
    bus.memory[0x1000] = 0xA9;
    bus.memory[0x1001] = 0x42;
    bus.memory[0x1002] = 0xAA;

    let mut cpu = CPU::new(bus);
    
    // Buffer to capture debug output
    type TraceRecord = (&'static str, u8, u8);
    let mut trace = std::vec::Vec::<TraceRecord>::new();

    // Define a static-friendly hook function
    fn debug_trace_hook<B: Bus>(cpu: &CPU<B>, user_data: *mut core::ffi::c_void) {
        let instr = &INSTRUCTION_TABLE[cpu.ir as usize];
        let trace = unsafe { &mut *(user_data as *mut std::vec::Vec<TraceRecord>) };
        trace.push((instr.mnemonic, cpu.a, cpu.x));
    }

    // Attach the debug hook and user data
    cpu.debug_hook = Some(debug_trace_hook);
    cpu.debug_user_data = &mut trace as *mut _ as *mut core::ffi::c_void;

    // 1. Execute Reset (8 cycles)
    for _ in 0..8 { cpu.tick(); }
    
    // 2. Execute LDA #$42 (2 cycles)
    // Cycle 1: Fetch A9
    cpu.tick(); 
    // Cycle 2: Fetch 42 & Load A. Hook should fire here.
    cpu.tick(); 
    
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0], ("LDA", 0x42, 0x00));

    // 3. Execute TAX (2 cycles)
    // Cycle 1: Fetch AA
    cpu.tick();
    // Cycle 2: Dummy Read & Transfer A to X. Hook should fire here.
    cpu.tick();

    assert_eq!(trace.len(), 2);
    assert_eq!(trace[1], ("TAX", 0x42, 0x42));
}
