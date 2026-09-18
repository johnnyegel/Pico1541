# Specification: Manager Abstraction Cleanup

## Overview
The `pico1541-manager` crate currently relies heavily on `pub static` globals for inter-task communication and synchronization. This creates tight coupling, exposes internal implementation details, and makes unit testing difficult. This specification outlines the removal of naked globals in favor of zero-cost Dependency Injection (DI) using Embassy's channel endpoints and shared references.

## 1. Dead Code Eradication
- Remove `SETTINGS_SAVE_RESULT` and `SETTINGS_SAVE_REQUEST` (`AtomicU8`) from `crates/pico1541-manager/src/lib.rs`.
- Remove the obsolete export `pub use pico1541_common::interface::DriveEvent;` from `lib.rs`.

## 2. Encapsulating `MOTOR_STATE`
- **Current State:** `pub static MOTOR_STATE: AtomicBool` in `mount_manager/mod.rs`.
- **Target State:** Remove the global static.
  - Define `static MOTOR_STATE: AtomicBool` in `pico1541-rp2350/src/main.rs` and `pico1541-rp2040/src/main.rs`.
  - Pass `&'static AtomicBool` to the constructors of both `MountControllerTask` and `TrackCacheTask`.

## 3. Channel Endpoint Injection
- **Current State:** Tasks hardcode calls to `crate::CMD_CHANNEL`, `crate::KEYBOARD_CHANNEL`, `crate::ACTION_CHANNEL`, and `crate::SETTINGS_CHANNEL`.
- **Target State:** Tasks must accept `embassy_sync::channel::Receiver` or `Sender` in their `new()` constructors.
  - The channels themselves will remain in `lib.rs` (or `main.rs`) but they should ideally not be accessed directly by task implementations.
  - *Note:* To avoid complex generic propagation for the mutex type across the entire codebase, we will standardize on `CriticalSectionRawMutex` for the endpoint types, e.g., `Receiver<'static, CriticalSectionRawMutex, InternalCommand, 128>`.

### Affected Tasks:
1. **`SettingsSaveTask`**:
   - Add `rx: Receiver<'static, CriticalSectionRawMutex, SettingsCommand, 2>` to `new()`.
2. **`MountControllerTask`**:
   - Add `cmd_rx: Receiver<'static, CriticalSectionRawMutex, InternalCommand, 128>` to `new()`.
3. **`KeyboardTask`** (`tasks/keyboard.rs`):
   - Add `event_tx: Sender<'static, CriticalSectionRawMutex, ButtonEvent, 16>` to `new()`.
4. **`ActionTask`** (`tasks/action.rs`):
   - Add `event_rx: Receiver<'static, CriticalSectionRawMutex, ButtonEvent, 16>` and `action_tx: Sender<'static, CriticalSectionRawMutex, UiAction, 16>` to `new()`.
5. **`MainMenuTask`** (`tasks/ui/mod.rs`):
   - Needs a `UiContext` struct to hold its transmitters, rather than passing 3-4 separate arguments.
   - `UiContext` should contain:
     - `action_rx: Receiver<'static, CriticalSectionRawMutex, UiAction, 16>`
     - `cmd_tx: Sender<'static, CriticalSectionRawMutex, InternalCommand, 128>`
     - `settings_tx: Sender<'static, CriticalSectionRawMutex, SettingsCommand, 2>`
   - This context must be passed down to the sub-views (`Browser`, `Settings`, etc.).

## 4. `main.rs` Wiring
Update both `rp2040` and `rp2350` `main.rs` to pass the `.sender()` and `.receiver()` endpoints to all instantiated tasks, fulfilling the new dependency injection requirements.
