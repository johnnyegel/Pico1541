# Implementation Plan: Manager Abstraction Cleanup

Based on: `docs/specs/manager_abstraction_cleanup.md`

## Task 1: Clean up Global Atomics and Exports
**Goal**: Remove dead code and encapsulate `MOTOR_STATE`.
- [ ] Remove `SETTINGS_SAVE_RESULT` and `SETTINGS_SAVE_REQUEST` from `crates/pico1541-manager/src/lib.rs`.
- [ ] Remove `pub use pico1541_common::interface::DriveEvent;` from `lib.rs`.
- [ ] Move `pub static MOTOR_STATE: AtomicBool` from `mount_manager/mod.rs` to a private static in both `main.rs` files (or pass it in). Wait, Embassy `Task` requires a `&'static` reference. We can just define `static MOTOR_STATE: AtomicBool = AtomicBool::new(false);` in both `main.rs`.
- [ ] Update `MountControllerTask::new` and `TrackCacheTask::new` to accept `motor_state: &'static AtomicBool` and store/use it instead of the global.
- [ ] Update both `main.rs` files to pass `&MOTOR_STATE` to the tasks.
- [ ] Verify `cargo check` passes for both targets.

## Task 2: Inject Channels into Background Tasks
**Goal**: Pass Sender/Receiver endpoints to `SettingsSaveTask`, `MountControllerTask`, `KeyboardTask`, and `ActionTask`.
- [ ] Update `SettingsSaveTask::new` and `run` to use injected `Receiver`.
- [ ] Update `MountControllerTask::new` and `run` to use injected `Receiver`.
- [ ] Update `KeyboardTask::new` and `run` to use injected `Sender`.
- [ ] Update `ActionTask::new` and `run` to use injected `Receiver` and `Sender`.
- [ ] Update both `main.rs` files to pass the `.receiver()` and `.sender()` endpoints from the globals.
- [ ] Verify `cargo check` passes for both targets.

## Task 3: Encapsulate UI Channels into `UiContext`
**Goal**: Create a unified context for the UI components so they don't rely on global channels.
- [ ] Define `UiContext` struct in `tasks/ui/mod.rs` containing `action_rx`, `cmd_tx`, and `settings_tx`.
- [ ] Update `MainMenuTask::new` and `run` to accept `UiContext` instead of using globals.
- [ ] Pass the `UiContext` (or specific endpoints) down to `Browser`, `Settings`, `VirtualDirectory`, etc., where they used the globals.
- [ ] Update both `main.rs` files to construct and pass `UiContext`.
- [ ] Run `cargo build --release` on both targets to verify the entire dependency injection tree is clean.
