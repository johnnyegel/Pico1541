# In-Memory Directory Child Resolution & Authoritative LED Architecture

## 1. Problem Statement & Motivation
In commit `b94264d`, `find_child` was introduced to resolve mangled C64 names (> 16 chars) by querying storage via `FileOpener::list_dir_range(dir_path, 0, 1000, ...)`.
This caused severe regressions:
1. **Concurrent SPI Bus Contention & Priority Inversion:** `MountControllerTask` (running on thread mode) and `TrackCacheTask` (running on `EXECUTOR_HIGH`, interrupt priority `P2`) both competed for the SD card `StorageWrapper` lock during real-time IEC bus transactions. This resulted in timeouts, mutex contention, and empty virtual directory listings.
2. **Scattered Imperative LED State:** The RGB status LED mode was set imperatively across multiple independent locations (`MountControllerTask`, `browser.rs`, `boot.rs`), leading to state mismatches where `DriveLocation::Root` displayed Blue (`StatusLedMode::VirtualDisk`) instead of Red (`StatusLedMode::Unmounted`).

## 2. Architectural Solution
1. **In-Memory Directory Child Resolution:**
   - When `TrackCacheTask` populates `PagedDirectorySource<144>`, it already receives each `FileInfo` (with full `name: [u8; 64]`).
   - Store entries whose filename exceeds 16 bytes in an in-memory `long_names: heapless::Vec<LongNameEntry, 64>` inside `PagedDirectorySource`.
   - Provide `paged.resolve_child(req_petscii)` and `paged.child_at_slot(slot)` to look up children **instantly in RAM** without touching the SD card or SPI bus.
   - For names $\le 16$ characters, decode PETSCII to ASCII directly from `DirectoryEntry.filename`.
   - Completely eliminate `find_child` and storage scanning from the `CD` command execution path.
2. **Authoritative LED Driver in `TrackCacheTask`:**
   - Remove `opener` from `MountControllerTask`.
   - `TrackCacheTask` owns `self.leds: StatusLedsWrapper` (or `L: RgbLedDriver`).
   - In `TrackCacheTask::run()`, whenever `current_loc != self.last_location`:
     - Update emulated track data (`mount_root` or `mount_sequence`).
     - Set `self.leds.set_status(status_led_for_location(&current_loc))`:
       - `DriveLocation::Root` $\rightarrow$ `StatusLedMode::Unmounted` (**Red**)
       - `DriveLocation::VirtualDirectory` $\rightarrow$ `StatusLedMode::VirtualDisk` (**Blue**)
       - `DriveLocation::DiskImage` (.d64) $\rightarrow$ `StatusLedMode::NonAccurateDisk` (**Green**)
       - `DriveLocation::DiskImage` (.g64 / .nib) $\rightarrow$ `StatusLedMode::AccurateDisk` (**Green**)
3. **Dispatch Commands to `TrackCacheTask`:**
   - Handle `InternalCommand::Cd` in `TrackCacheTask` using in-memory resolution against `vdir.source`.

## 3. Tasks & Implementation Steps
- [ ] **Task 1: Add `LongNameEntry` and in-memory resolution to `PagedDirectorySource`**
  - Define `LongNameEntry { slot: u8, name: heapless::String<64> }` in `storage.rs`.
  - Add `long_names` storage to `PagedDirectorySource`.
  - Update `FileOpener::list_dir_range` callback to pass `(DirectoryEntry, &str)`.
  - Add `resolve_child(&self, req_petscii: &[u8])` and `child_at_slot(&self, slot: usize)`.
- [ ] **Task 2: Route `CD` resolution in RAM within `TrackCacheTask`**
  - Remove `find_child` and storage access from `cd_handler` / `resolve_child_location`.
  - Use in-memory resolution in `TrackCacheTask` for both `CD` commands and `select_virtual_file`.
  - Remove `opener` from `MountControllerTask`.
- [ ] **Task 3: Authoritative LED State Management**
  - Implement `status_led_for_location(loc: &DriveLocation) -> StatusLedMode`.
  - Update LED strictly on location changes in `TrackCacheTask`.
  - Remove scattered manual `set_status()` calls in `browser.rs` and `MountControllerTask`.
- [ ] **Task 4: Verification & Hardware Testing**
  - Run all unit and integration tests (`cargo test`).
  - Verify RP2040 and RP2350 release builds with 0 compiler warnings.
