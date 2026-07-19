# Fast Loader Debugging Strategy & Task Tracker

## Background
The Pico1541 emulator functions correctly during standard C64 routines. However, fast loaders struggle or fail. It is currently unclear if the root cause is a raw performance bottleneck (i.e., the emulator falling behind real-time 1MHz execution) or a logic/timing exactness issue (e.g., inaccurate interrupt timing, VIA shift register behavior, or bus state propagation). Adding the flight recorder for debugging introduces significant overhead, artificially causing performance to drop below 1MHz.

## Goal
Systematically identify and resolve the root cause of the fast loader failures without introducing observer effects (Heisenberg debugging).

---

## Task Tracker & Strategy

### Phase 1: Measure, Don't Guess (Isolate the Bottleneck)
Before optimizing code, we must definitively prove whether the emulator is actually dropping below 1MHz during fast loaders.

- [x] **Restore `pico1541-perf` crate functionality**: Ensure performance tests build and run against the latest `ViaHardware` interfaces.
- [ ] **Run `pico1541-perf` on hardware**: Flash the perf tests using `probe-rs run` or `cargo test --release` and log the CPU cycle counts to verify baseline throughput.
- [ ] **Log Performance Counters**: Hook up `hw.report_performance_counters(cycle_counter, idle_counter)` in the `HardwareBridge` implementation. Have the Manager (Core 0) periodically print these counters to USB serial.
  - *Metric:* If `idle_counter` stays at 0 while `cycle_counter` falls behind real-time, it is a throughput bottleneck.
  - *Metric:* If `idle_counter` consistently increases, performance is fine, indicating a logic/cycle-exactness bug.

### Phase 2: Zero-Overhead Hardware-Assisted Debugging
If throughput is sufficient, the issue is timing/logic. We must observe the system without slowing it down.

- [ ] **Enhance the IEC Snooper**: Modify the `IecSnooper` (running on Core 0) to timestamp all ATN/CLK/DATA transitions. This provides a logic analyzer view of host-to-drive communication with zero impact on Core 1's emulation speed.
- [ ] **GPIO Logic Analyzer Hook (Optional but recommended)**: Toggle unused GPIO pins (e.g., High on entering `execute_instruction`, Low on exit) and monitor with an external logic analyzer. This costs 1 CPU cycle and provides absolute ground truth of timing and CPU load relative to IEC bus states.

### Phase 3: Optimize the Flight Recorder
If internal CPU state (A, X, Y, PC) is required to debug the crash, the flight recorder must be optimized to run at 1MHz real-time.

- [ ] **Filter State Recording**: Only record state transitions on I/O operations (VIA reads/writes or IEC pin changes) instead of every single instruction.
- [ ] **Memory Allocation**: Ensure the flight recorder buffer is allocated in a dedicated, uncontended SRAM bank using `#[link_section = ".uninit"]` to prevent cache thrashing.

### Phase 4: Memory Introspection (RAM Dumping)
Establish a reliable method to dump the emulator's memory state upon failure.

- [ ] **Implement Halt & Dump mechanism**: Add a host command via the Core 0 Manager (e.g., via USB serial). Upon receipt, Core 0 sends a `Halt` event to Core 1. Core 1 halts, allowing Core 0 to safely read the 64KB `bus.memory` (placed in a `Mutex` or `UnsafeCell`) and print it to the terminal in hex format.

---

## Execution Log
*Keep a running log of findings and decisions here as tasks are completed.*

**2026-07-19:**
- Updated and fixed `pico1541-perf` test suites to pass the newly required zero-cost trait mocks. Validated compilation for `thumbv6m-none-eabi`.
- Drafted this strategy document.
