# PIO Magnetic Surface Emulation Design

## Objective
Refactor the cycle-exact 1541 emulator to offload `.g64` bit-level alignment and SYNC detection to the RP2040 PIO, while separating hardware-specific implementations from the agnostic drive mechanics via zero-cost static dispatch. This ensures true cycle accuracy and correctly handles C64 copy protections.

## Architecture: The `MagneticSurface` Trait
A boundary trait will be defined in `pico1541-common` to cleanly separate bit-shifting from `DriveMechanics`.

```rust
pub trait MagneticSurface {
    /// Checks if the read head is currently passing over a SYNC zone.
    fn is_sync_active(&self) -> bool;
    
    /// Pulls a perfectly aligned GCR byte from the hardware FIFO, if available.
    fn try_read_byte(&mut self) -> Option<u8>;
    
    /// Calculates physical byte index based on a 200,000-cycle 300 RPM rotation.
    fn get_absolute_head_position(&self, master_clock: u32) -> u32;
    
    /// Executes a direct RAM overwrite for a specific byte on the track.
    fn write_byte_at(&mut self, index: u32, data: u8);
    
    /// Commands hardware to resume spinning from a specific physical offset.
    fn start_reading(&mut self, track_id: u8, rotation_percentage: f64);
    
    /// Aborts DMA/PIO for write transitions.
    fn stop_reading(&mut self);
}
```

*Virtual Tracks Consideration: This trait hides the underlying storage mechanism. It natively supports Virtual Tracks (e.g., dynamically generated GCR) because `DriveMechanics` only ever consumes bytes via `try_read_byte()` without knowing if the DMA is feeding a static track array or a dynamically generated ping-pong buffer.*

### Implementations
1. **`PioMagneticSurface`** (in `pico1541-rp2040`): Communicates with the PIO State Machine, DMA channels, and shared atomic variables.
2. **`SoftwareMagneticSurface`** (in `pico1541-common`): Retains the legacy `EmulatorHandle` bit-shifting logic. This struct is specifically built for pure PC emulation and unit tests (`disk_streaming.rs`), ensuring tests don't break when `DriveMechanics` expects a `MagneticSurface`.

## RP2040 DMA Ping-Pong Chaining
To feed arbitrary-length `.g64` track arrays into the PIO infinitely without CPU intervention, we will use a two-channel DMA setup utilizing the AL3 alias registers:
- **Worker Channel (A)**: Transfers bytes from the `TrackBuffer` base address to the PIO TX FIFO. `TRANS_COUNT` is set to the track length. `CHAIN_TO` points to Channel B.
- **Reloader Channel (B)**: Configured to transfer exactly 2 words. It reads from a static `[TRANS_COUNT, READ_ADDR]` array and writes to Channel A's `AL3_TRANS_COUNT` and `AL3_READ_ADDR_TRIG` registers. Channel B uses a `RING` on its read address to infinitely loop over this 2-word control block.

## PIO State Machine
The `pioasm` script acts as the read controller and operates in two main loops to discard garbage bytes and align GCR cleanly:

1. **SYNC_SEARCH**: 
   - Pulls a bit from the OSR (fed by DMA).
   - If `0`: Sets `X` to 9, shifts `0` into the ISR.
   - If `1`: Shifts `1` into the ISR, decrements `X`. If `X` reaches 0 (10 consecutive ones), it jumps to `IN_SYNC`.
2. **IN_SYNC**:
   - Asserts `IRQ 0` (shared atomic flag for `is_sync_active`).
   - Executes `mov isr, null` (discards accumulated bits and resets the autopush counter).
   - Pulls a bit. If `1`, it loops here. If `0`, it clears `IRQ 0`, sets `X` to 9, shifts `0`, and jumps to `SYNC_SEARCH`.

*The ISR is configured with an Autopush of 8 bits, meaning perfectly aligned bytes will automatically drop into the RX FIFO only when in the `SYNC_SEARCH` state.*

## `DriveMechanics` Refactoring
The hot-loop inside `drivemechanics.rs` will be updated to consume the new trait.
- **Read Mode**: The timer checks `surface.is_sync_active()`. If false, it calls `surface.try_read_byte()`. If `Some(byte)` is returned, the SO pulse is fired.
- **Write Mode Transition**: Upon detecting a transition to write mode via VIA CB2, `surface.stop_reading()` is called. The current anchor byte is calculated via `get_absolute_head_position()`.
- **Write Mode**: Bytes are written directly to RAM via `surface.write_byte_at(index, data)`.
