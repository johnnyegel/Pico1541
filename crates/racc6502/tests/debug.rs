

#[cfg(feature = "debug")]
struct DebugBus {
    memory: [u8; 65536],
}

#[cfg(feature = "debug")]
impl Bus for DebugBus {
    type Ctx = ();
    fn read(&mut self, addr: u16, _ctx: &mut ()) -> u8 {
        self.memory[addr as usize]
    }
    fn write(&mut self, addr: u16, val: u8, _ctx: &mut ()) {
        self.memory[addr as usize] = val;
    }
}

#[test]
#[cfg(feature = "debug")]
fn test_debug_hook_on_load() {
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

    // Buffer to capture debug output (Opcode, A, X)
    type TraceRecord = (u8, u8, u8);
    let mut trace = std::vec::Vec::<TraceRecord>::new();

    // Define a static-friendly hook function
    fn debug_trace_hook(cpu: &CPU, user_data: *mut core::ffi::c_void) {
        let trace = unsafe { &mut *(user_data as *mut std::vec::Vec<TraceRecord>) };
        trace.push((cpu.ir, cpu.a, cpu.x));
    }

    // Attach the debug hook and user data
    cpu.debug_hook = Some(debug_trace_hook);
    cpu.debug_user_data = &mut trace as *mut _ as *mut core::ffi::c_void;

    // 1. Execute Reset (8 cycles)
    // Reset does not fire the hook (it goes from Reset state directly to FetchOpcode)
    for _ in 0..8 {
        cpu.tick(&mut bus, &mut ());
    }

    assert_eq!(trace.len(), 0);

    // 2. Execute LDA #$42 (2 cycles)
    // First cycle of LDA #$42 is the FetchOpcode cycle. Hook fires here.
    cpu.tick(&mut bus, &mut ()); // Fetch A9. IR becomes 0xA9. Hook fires.

    assert_eq!(trace.len(), 1);
    // At this point, A is still 0 because LDA hasn't executed its operation yet.
    assert_eq!(trace[0], (0xA9, 0x00, 0x00));

    cpu.tick(&mut bus, &mut ()); // Execute LDA operation (loads A with 42)

    // 3. Execute TAX (2 cycles)
    // First cycle of TAX is FetchOpcode. Hook fires here.
    cpu.tick(&mut bus, &mut ()); // Fetch AA. IR becomes 0xAA. Hook fires.

    assert_eq!(trace.len(), 2);
    // Now A is 0x42 (from previous instruction), but X is still 0.
    assert_eq!(trace[1], (0xAA, 0x42, 0x00));

    cpu.tick(&mut bus, &mut ()); // Execute TAX operation (transfers A to X)

    // 4. Next instruction fetch would fire hook again
    cpu.tick(&mut bus, &mut ()); // Fetch next (0x00 / BRK)
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[2], (0x00, 0x42, 0x42));
}
