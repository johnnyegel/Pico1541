# Architectural Improvement Plan - Pico1541 Emulator

This document outlines the strategic improvements to ensure the Pico1541 emulator maintains cycle-exactness, clean separation of concerns, and high testability while running on the RP2040.

## 1. Performance & Cycle-Exactness
### Zero-Cost Logging
- **Objective:** Retain debugging logs during development while ensuring zero runtime overhead in production firmware.
- **Action:** 
    - Use `log` crate features (e.g., `release_max_level_off`) or custom `cfg(feature = "logging")` gates.
    - Audit the `1 MHz` hot path (`SystemBus::tick_cycle`, `DriveMechanics::tick`) to ensure the compiler elides all formatting and dispatch logic in release builds.

### Jitter & Timing
- **Objective:** Guarantee a stable 1 MHz pulse on Core 0.
- **Action:** 
    - Review the `emulator_loop` catch-up mechanism.
    - Ensure that any occasional "long" cycles (e.g., complex GCR calculations) are compensated for in subsequent cycles to maintain phase alignment with the host.

## 2. Clean Separation of Concerns
### High-Level Emulation (HLE) Abstraction
- **Objective:** Remove DOS-specific "hacks" from the pure hardware emulation layer.
- **Action:**
    - Create an `HleManager` or `InterceptHandler` to manage ROM-specific interventions (like silencing "SYNTAX ERROR" after a hijack).
    - Refactor `SystemBus::discard_command_buffer` to use this new abstraction, keeping `SystemBus` focused strictly on memory mapping and signal routing.

### Hardware vs. Logic Boundaries
- **Objective:** Maintain the purity of the `pico1541-emulator` crate.
- **Action:** 
    - Ensure all interventions are triggered via explicit, well-defined hooks in the emulation loop.

## 3. Testability & Validation
### Cycle-Exact Assertions
- **Objective:** Empirically verify timing accuracy.
- **Action:**
    - Enhance `TestBridge` to track total elapsed cycles.
    - Update unit tests to assert `cycles()` counts for critical operations (e.g., "CD:" hijack detection latency).

### Component Isolation
- **Objective:** Test the `IecSnooper` and `HleManager` independently of the full 6502 state where possible.

## 4. Optimization
- **Action:**
    - Profile the `racc6502` micro-op engine.
    - If overhead is high, refactor the dispatch mechanism to use a faster table-based or direct `match` approach.
