# Plan: Fix Directory Listing Boot Tests

## Objective
Resolve the failures in `crates/pico1541-emulator/tests/boot.rs`, specifically `test_dir_listing_g64` (Path error) and `test_dir_no_disk`/`test_dir_listing_virtual` (Timeout).

## Analysis

### 1. `test_dir_listing_g64` Path Error
- **Issue:** The test attempts to mount `tests/data/test2.g64`, but the actual file in the repository is `tests/data/test.g64`.
- **Solution:** Update the file path in `boot.rs` to point to the correct test data.

### 2. `test_dir_no_disk` Timeout
- **Issue:** The test times out waiting for the drive to respond to the `$` (directory) command. 
- **Hypothesis:** When `disk_inserted` is false, the 1541 ROM may enter an infinite loop waiting for SYNC or a specific hardware state that isn't being correctly emulated or triggered in the `TestBridge`.
- **Solution:** 
    - Verify that `DriveMechanics` correctly reports "No Disk" status.
    - Ensure `TestBridge` provides appropriate "noise" or state when no disk is present to allow the ROM to exit wait loops with a "74, DRIVE NOT READY" error rather than hanging.

### 3. `test_dir_listing_virtual` Timeout
- **Issue:** Similar to the no-disk case, but here a virtual disk is present.
- **Hypothesis:** The virtual disk data (Track 18) might not be delivering the expected SYNC signals or the `TestBridge` might not be ticking the drive enough between IEC operations.
- **Solution:**
    - Increase the inter-command tick count if necessary.
    - Validate that the `VirtualDirImage` generated GCR data contains valid SYNC ($FF bytes) and that `DriveMechanics` is correctly asserting the SYNC line.

## Implementation Steps

1. **Path Correction:** Fix the G64 file path in `test_dir_listing_g64`.
2. **Debug Instrumentation:** Add logging to the `TestBridge` and `DriveMechanics` to track CPU execution and SYNC detection during the hanging tests.
3. **ROM Wait-Loop Analysis:** Identify the specific ROM address where the CPU is hanging (using `drive.pc()`).
4. **Behavioral Fix:** Adjust `TestBridge` or `DriveMechanics` to ensure the ROM receives the signals necessary to proceed (either with data or an error).
5. **Validation:** Run the full `boot.rs` suite and verify all directory listing tests pass.
