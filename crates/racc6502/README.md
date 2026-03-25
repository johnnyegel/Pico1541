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
