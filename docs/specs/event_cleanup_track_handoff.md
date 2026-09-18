# Event Handling Cleanup & Track Handoff Architecture

## 1. Overview
The current event loop architecture in the Manager crate suffers from silent deadlocks caused by coupling synchronous SD card reads, background timer polling, and asynchronous command/track signals inside a single `select3` macro. Additionally, the event channel system contains substantial dead code (Snooper and DriveActivity tasks) that clogs the architecture without providing runtime functionality.

This plan aims to:
1. Eradicate dead event handling code.
2. Abstract Core-to-Core track notifications into a zero-cost `TrackHandoff` hardware doorbell.
3. Decouple the Manager into independent tasks to eliminate deadlocks.

---

## 2. Phase 1: Eradication of Dead Code
The `EVENT_CHANNEL` and `ACTIVITY_CHANNEL` are currently unused on hardware because the emulator never calls `send_event()`. 
- **Tasks to Remove:** Delete `SnooperTask` and `DriveActivityTask` from `pico1541-manager`. (The Snooper logic will be revived in a future iteration when full emulation is ready).
- **Channels to Remove:** Delete `EVENT_CHANNEL` and `ACTIVITY_CHANNEL`.
- **Command Pruning:** Strip `InternalCommand` in `CMD_CHANNEL`. Remove `LoadFile` and `CloseChannel`. Retain only `Mount`, `Unmount`, and `MotorState`.

---

## 3. Phase 2: The Track Handoff Abstraction
The current cross-core track signaling relies on a global `AtomicU8` and an Embassy `Signal`, manually forced by writing to the RP2040/RP2350 `TIMER0` interrupt force register (`INTF`). This exposes hardware details to the generic Manager crate.

We will introduce a `TrackHandoff` abstraction:
- **Trait Definition (`pico1541-common`):**
  ```rust
  pub trait TrackHandoff {
      /// Called by Core 1 (Emulator) when the head steps to a new track.
      fn notify_track_change(&mut self, half_track: u8);
      
      /// Returns the most recently requested track.
      fn current_track(&self) -> u8;
      
      /// Awaited by Core 0 (Manager). Completes when a new track is notified.
      async fn wait_for_change(&mut self) -> u8;
  }
  ```
- **Hardware Implementation (`pico1541-rp-common`):**
  Implement `RpTrackHandoff` using the underlying `AtomicU8` and hardware interrupt force logic. This fully hides the RP2040/RP2350 specific doorbell mechanism (whether using `INTF` or SIO FIFOs) from the Manager.

---

## 4. Phase 3: Decoupling the Mount Manager
The monolithic `MountManagerTask` will be split into two focused, independent tasks running on Core 0.

### A. The Track Cache Task (High Priority)
Responsible solely for pre-fetching data from the SD card to RAM.
- **Run Loop:** Uses a simple `select` between `TrackHandoff::wait_for_change()` and `embassy_time::Timer::after_millis(20)`.
- **Execution:** When the doorbell rings, it immediately aborts any background precaching and focuses on loading the requested track. During idle time (the 20ms timer ticks), it performs background precaching and directory buffer streaming.

### B. The Mount Controller Task (Standard Priority)
Responsible for handling disk swaps, UI commands, and motor states.
- **Run Loop:** Awaits `CMD_CHANNEL.receive()`.
- **Execution:** Handles `Mount(Path)`, `Unmount`, and `MotorState(bool)`. By moving this out of the track caching loop, UI interactions can never be starved by long SD card reads, and the task will never encounter the silent `select3` deadlocks.

---

## 5. Execution Steps
1. Delete the dead tasks and channels.
2. Build the `TrackHandoff` abstraction and inject it into the Emulator and Manager.
3. Refactor `MountManagerTask` into the decoupled architecture.
4. Verify functionality by booting the "Edge of Disgrace" demo and testing disk swaps during heavy track loading.
