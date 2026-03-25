# racc6502

A cycle-accurate, `#![no_std]` MOS 6502 CPU emulator written in Rust.

## Architecture: Micro-Op State Machine

Unlike traditional emulators that execute an entire instruction in a single function call, `racc6502` uses a **Micro-Op (uOp) state machine**. This design ensures that every clock cycle is represented individually, allowing for perfect synchronization with external hardware (like the VIA 6522 or custom PIO logic in the Pico1541 project).

### Key Features

*   **Cycle-Exact Timing**: A single call to `CPU::tick()` performs exactly one internal micro-operation and exactly one bus access (read or write).
*   **Micro-Op Slices**: Every opcode is mapped to two static slices:
    *   `AddrMicroOp`: Handles the addressing mode (fetching low/high bytes, indexing, page-cross dummy reads).
    *   `ExecMicroOp`: Handles the instruction execution (ALU operations, stack pushes, jumps).
*   **Dummy Cycle Fidelity**: Correctly replicates NMOS 6502 behavior, including reading from intermediate addresses during page-crosses and performing dummy writes during Read-Modify-Write instructions.
*   **No Standard Library**: Designed for embedded environments (like the RP2040). Zero allocations, zero `std`.
*   **Hardware Signal Support**: Fully implements `IRQ`, `NMI`, and the `SO` (Set Overflow) pin.
*   **Decimal Mode**: Cycle-accurate NMOS decimal mode for `ADC` and `SBC`.

## Debugging

`racc6502` provides a zero-overhead (in release) debugging hook system. This allows you to inspect the CPU state at every "Sync" point (the moment an instruction completes).

### Instruction Mnemonics
In debug builds, the `Instruction` struct includes a `mnemonic` field (e.g., `"LDA"`, `"TAX"`). This is automatically populated from the instruction table.

### Using the Debug Hook
You can attach a function pointer to the CPU that will be invoked after every instruction. To maintain `no_std` compatibility without closures, the hook accepts a `user_data` pointer for state management.

```rust
use racc6502::instructions::nmos6502::INSTRUCTION_TABLE;

// 1. Define a stateful trace record
struct Trace {
    mnemonic: &'static str,
    a: u8,
}

// 2. Define the hook function
fn my_trace_hook<B: Bus>(cpu: &CPU<B>, user_data: *mut core::ffi::c_void) {
    let trace_log = unsafe { &mut *(user_data as *mut Vec<Trace>) };
    let instr = &INSTRUCTION_TABLE[cpu.ir as usize];
    
    trace_log.push(Trace {
        mnemonic: instr.mnemonic,
        a: cpu.a,
    });
}

// 3. Attach to the CPU
let mut log = Vec::new();
cpu.debug_hook = Some(my_trace_hook);
cpu.debug_user_data = &mut log as *mut _ as *mut core::ffi::c_void;
```

## Verification

This emulator is verified against **Klaus Dormann's 6502 functional test suite**. It successfully passes all 151 legal NMOS opcodes, reaching the success trap at `$3469`.

### Running Tests

```bash
# Run unit tests
cargo test

# Run the full functional test suite (takes ~40s)
cargo test --test functional -- --ignored
```

## Integration

To use the CPU, implement the `Bus` trait for your memory/device mapping:

```rust
use racc6502::bus::Bus;
use racc6502::cpu::CPU;

struct MySystem { memory: [u8; 65536] }

impl Bus for MySystem {
    fn read(&mut self, addr: u16) -> u8 { self.memory[addr as usize] }
    fn write(&mut self, addr: u16, val: u8) { self.memory[addr as usize] = val; }
}

let bus = MySystem { memory: [0; 65536] };
let mut cpu = CPU::new(bus);

// Execute one clock cycle
cpu.tick();
```
