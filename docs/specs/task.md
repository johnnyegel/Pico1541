# Pico1541 Emulation Architecture Refactoring

## 1. Explore project context
We are refactoring the execution and hardware synchronization model of the `pico1541-emulator` to eliminate timing inaccuracies that caused IEC bus hangs. The coarse batch execution model caused the CPU to run too far ahead, missing tight physical timing windows and I/O states.

## 2. Assess visual needs
Purely architectural and backend refactoring task. No visual components.

## 3. Ask clarifying questions
- Confirmed: VIA read correctly combines hardware state and output register state.
- Confirmed: Hardware uses 1 for Asserted (7405 inverters).
- Confirmed: Synchronization uses absolute wrapping timer (`pac::TIMER.timerawl()`).
- Confirmed: Asynchronous events (ATN) polled directly from physical GPIOs during sync.

## 4. Propose 2-3 approaches
**Selected Approach: The "Jitter-Tolerant Catch-Up" Model**
- **CPU Loop:** Executes one instruction at a time.
- **Absolute Time Tracking:** CPU maintains a 32-bit `cycle_counter`. The baseline is the `timerawl()` at boot.
- **Sync Logic:** `sync_and_poll(cycle_counter)` is called after every instruction and during `bus.read()` / `bus.write()`. It spinlocks while `timerawl() - baseline < cycle_counter`. 
- **Handling Heavy Loads:** If `drivemechanics` takes 50µs to process (overshooting the 2µs instruction window), the CPU simply falls *behind* real time. The next several `sync_and_poll` calls will return instantly (since real time is now > CPU time), allowing the 250MHz RP2040 to burst-execute 6502 instructions until it catches back up to the real-time clock. This provides a natural buffer for heavy operations without breaking the absolute timeline.
- **Hardware Trait:** `ViaHardware` trait handles physical GPIO interactions, decoupling the emulator from the RP2040 HAL.

## 5. Present design
*Complete - See above.*

## 6. Write design doc
*Pending - We will draft the Rust trait structures next.*

## 7. Spec self-review
*Pending.*

## 8. User reviews written spec
*Pending.*

## 9. Transition to implementation
*Pending.*
