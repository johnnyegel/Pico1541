use racc6502::cpu::CPU;
use racc6502::bus::Bus;
use racc6502::instructions::nmos6502::INSTRUCTION_TABLE;

struct DebugBus {
    memory: [u8; 65536],
}

impl Bus for DebugBus {
    type Ctx = ();
    fn read(&mut self, addr: u16, _ctx: &mut ()) -> u8 { self.memory[addr as usize] }
    fn write(&mut self, addr: u16, val: u8, _ctx: &mut ()) { self.memory[addr as usize] = val; }
}

#[test]
#[cfg(feature = "debug")]
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

    let mut cpu = CPU::new();

    // Buffer to capture debug output
    type TraceRecord = (&'static str, u8, u8);
    let mut trace = std::vec::Vec::<TraceRecord>::new();

    // Define a static-friendly hook function
    fn debug_trace_hook(cpu: &CPU, user_data: *mut core::ffi::c_void) {
        let instr = &INSTRUCTION_TABLE[cpu.ir as usize];
        let trace = unsafe { &mut *(user_data as *mut std::vec::Vec<TraceRecord>) };
        trace.push((instr.mnemonic, cpu.a, cpu.x));
    }

    // Attach the debug hook and user data
    cpu.debug_hook = Some(debug_trace_hook);
    cpu.debug_user_data = &mut trace as *mut _ as *mut core::ffi::c_void;

    // 1. Execute Reset (8 cycles)
    for _ in 0..8 { cpu.tick(&mut bus, &mut ()); }

    // 2. Execute LDA #$42 (2 cycles)
    cpu.tick(&mut bus, &mut ()); // Fetch A9
    cpu.tick(&mut bus, &mut ()); // Fetch 42 & Load A. Hook should fire here.

    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0], ("LDA", 0x42, 0x00));

    // 3. Execute TAX (2 cycles)
    cpu.tick(&mut bus, &mut ()); // Fetch AA
    cpu.tick(&mut bus, &mut ()); // Dummy Read & Transfer A to X. Hook should fire here.

    assert_eq!(trace.len(), 2);
    assert_eq!(trace[1], ("TAX", 0x42, 0x42));
}
