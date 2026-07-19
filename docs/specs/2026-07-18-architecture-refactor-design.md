# Pico1541 Emulation Architecture Refactoring

## 1. Objective
To completely resolve timing issues between the 6502 emulator and the physical IEC bus by replacing the current coarse batch-execution model with an exact-cycle "Catch-Up and Poll" execution model. This moves the emulator from a "poll host state occasionally" design to a cycle-accurate hardware-linked design.

## 2. Core Architectural Changes

### 2.1 The `CatchUpContext`
A new synchronization context passed into all memory and I/O accesses. It maintains the CPU's exact execution cycle.

```rust
pub trait CatchUpContext {
    /// Blocks (spinlocks) the RP2040 until the physical hardware timer 
    /// catches up to the provided `target_cycle`.
    fn catch_up(&mut self, target_cycle: u64);
    
    /// Called at the end of every instruction to keep physical time perfectly 
    /// locked, preventing the CPU from ever drifting "into the future".
    fn advance_to(&mut self, target_cycle: u64);
}
```

### 2.2 Polling the GPIOs
Event channels for IEC state changes (like ATN transitions) will be completely removed from the execution path. Instead, the `catch_up` spinlock will directly read the physical RP2040 GPIO pins while it waits. If `ATN` asserts during the spinlock, it immediately raises the `CA1` flag on the VIA. This guarantees interrupt handling is cycle-accurate.

### 2.3 The `ViaHardware` Trait
The `pico1541-emulator` crate will be decoupled from physical GPIOs and the existing `HardwareBridge` via a new trait:

```rust
pub trait ViaHardware {
    fn read_pa(&mut self, ctx: &mut impl CatchUpContext) -> u8;
    fn read_pb(&mut self, ctx: &mut impl CatchUpContext) -> u8;
    fn write_pa(&mut self, ctx: &mut impl CatchUpContext, val: u8);
    fn write_pb(&mut self, ctx: &mut impl CatchUpContext, val: u8);
    fn set_ca1(&mut self, ctx: &mut impl CatchUpContext, active: bool);
    // ... implementations for CB1/CA2/CB2 as needed
}
```

When the 6502 reads or writes to a VIA port, the `ViaHardware` implementation guarantees that `ctx.catch_up()` is called *before* the physical GPIO is touched, ensuring the exact target microsecond.

### 2.4 Jitter-Tolerant Load Balancing
Because the synchronization is based on an absolute timer (`timerawl()` relative to a boot snapshot), the system is naturally jitter-tolerant. 
If a heavy operation in `drivemechanics` (like GCR formatting) takes 50µs to compute, the `catch_up` spinlock will overshoot. The next several instructions will call `catch_up`, but the spinlock will immediately fall through (since physical time > CPU time), allowing the 250MHz RP2040 to "burst execute" until it catches back up. This guarantees a perfect 1MHz average clock rate without crashing due to temporary processing spikes.

### 2.5 Relocating the Execution Loop
The `ExecutionLoop` (the `loop { cpu.step(); ... }`) will be entirely extracted from `pico1541-emulator/src/lib.rs` and moved into a dedicated module within the `pico1541-rp2040` crate (e.g. `src/emulator_loop.rs`). This clearly separates the pure state-machine of the emulator from the RP2040's hardware polling and synchronization logic.

## 3. Implementation Steps
1. Create `CatchUpContext` and `ViaHardware` traits in `pico1541-emulator`.
2. Refactor `SystemBus` and `VIA` instances to take these traits and `context` arguments during memory reads/writes.
3. Remove the old `HardwareBridge`, channel polling, and batch-execution timing logic.
4. Create the new `emulator_loop` module in `pico1541-rp2040` that implements the tight `catch_up` spinlock and instantiates the CPU.
5. Update unit tests in `pico1541-emulator` to use a mock `CatchUpContext` that immediately advances virtual time.
