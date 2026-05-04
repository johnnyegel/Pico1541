# G64 GCR Track Reading Validation Plan

## Goal
Verify that the `DriveMechanics` and `G64Image` handler correctly process real `.g64` images, generating accurate SYNC signals and bitstream data for the emulated 6502 CPU.

## Methodology
Implement a low-level, high-fidelity unit test that bypasses the full system emulation to focus strictly on the interaction between disk storage, drive mechanics, and the VIA 6522 interface.

## Implementation Details

### 1. Test Environment
- **File:** `crates/pico1541-emulator/tests/g64_read_validation.rs`
- **Data Source:** `crates/pico1541-emulator/tests/data/test2.g64`
- **Track:** Track 18.0 (Half-track 34).
- **Bitrate:** Zone 2 (28 cycles per byte).

### 2. Test Steps
1.  **Mount G64:** Use `G64Image` to load Track 18 from the test image into a buffer.
2.  **Initialize Components:**
    - `Via6522`: Configured with motor ON and Bitrate Zone 2.
    - `DriveMechanics`: Reset and positioned at half-track 34.
    - `TestBridge`: Mocked `HostInterface` providing the `ActiveTrack` data.
3.  **Spin-up Simulation:** Run for 250,000 cycles to allow `DriveMechanics` to reach "Spun Up" state.
4.  **Rotation Capture:**
    - Execute for 200,000 cycles (slightly more than one full rotation).
    - Capture every byte delivered to `via2.in_a`.
    - Monitor `via2.in_b` bit 7 for SYNC assertions.
    - Track `via2.set_ca1` calls (Byte Ready pulses).

### 3. Success Criteria
- **SYNC Detection:** Verify that `via2.in_b & 0x80` is high if and only if the current GCR byte is `$FF`.
- **Header/Data Framing:**
    - Every Header SYNC must be followed by a GCR byte `$08`.
    - Every Data SYNC must be followed by a GCR byte `$07`.
- **Sector Count:** Detect exactly 19 sectors (38 SYNC marks total for headers and data blocks).
- **Timing:** Verify that bytes are delivered exactly every 28 cycles.
- **SO Pin (Byte Ready):** Confirm that `CA1` (Byte Ready) does NOT pulse during SYNC, only for data bytes.

## Execution
1. Create the test file.
2. Implement a `G64Scanner` helper within the test to simplify rotation analysis.
3. Assert the count and validity of the captured blocks.
