# Firmware Weak-Bit Emulation (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement zero-overhead weak-bit disk emulation on RP2350 preserving 100% of the hardware PIO/DMA spinning disk architecture and cycle-exact 1 MHz 6502 hot path.

**Architecture:** When G64 tracks are mounted, the Manager detects `0x00` runs (weak bits) on the cold path, pre-populating them with noise and storing compact `WeakBitRange` markers in `TrackBuffer`. In the emulator, `try_read_ram_byte()` remains completely untouched. Outside the 512-tick batch, a lightweight check detects when DMA Channel 0 wraps its revolution and mutates only the recorded weak bytes in SRAM using a fast 16-bit Galois LFSR masked against false SYNC marks (`0xFF`).

**Architecture Diagram:**

```mermaid
graph TD
    subgraph "Manager (Cold Path on Track Load)"
        G64[G64 Image on SD] --> LoadTrack[Load Track into Dormant Buffer]
        LoadTrack --> ScanWeak[Scan for 0x00 Runs]
        ScanWeak --> SetRanges[Record WeakBitRanges & Pre-populate Noise]
        SetRanges --> FlipGen[Flip Active Generation]
    end

    subgraph "Core 0 Emulator Hot Path"
        TickLoop["512 Ticks Batch (UNTOUCHED)"]
        TickLoop --> ReadByte["try_read_ram_byte() (3-instruction FIFO check)"]
    end

    subgraph "End-of-Revolution Shadow Mutation (Outside Batch)"
        TickLoop --> CheckReload{"DMA CH0 trans_count Reloaded?"}
        CheckReload -->|No (99.7%)| NextBatch[Continue Emulation]
        CheckReload -->|Yes (Every 200 ms)| MutateSRAM["Mutate Weak Ranges in SRAM via Lfsr16"]
        MutateSRAM --> NextBatch
    end
```

**Tech Stack:**
- Rust (`no_std`)
- Embedded RP2350 (`thumbv8m.main-none-eabihf`)
- Embassy runtime (`embassy-rp`)
- 16-bit Galois LFSR PRNG

## Global Constraints
- **Zero hot path degradation:** No instructions or branching added to `try_read_ram_byte()`.
- **Preserve DMA loop:** Do not alter the DMA Channel A / Channel B ping-pong reload chain in `PioMagneticSurface`.
- **Compiler cleanliness:** Zero compiler warnings on `cargo clippy` and `cargo test`.
- **Target release build:** Must compile cleanly for `pico1541-rp2350` in release mode.

---

### Task 1: Core Data Structures & Weak Range Detection in `pico1541-common` and `pico1541-manager`

**Files:**
- Modify: `crates/pico1541-common/src/track_buffer.rs`
- Modify: `crates/pico1541-manager/src/formats/g64.rs`
- Test: `crates/pico1541-manager/tests/g64_weak_bits.rs` (or unit test in `g64.rs`)

**Interfaces:**
- Consumes: Track data read from G64 disk image.
- Produces:
  - `WeakBitRange { pub offset: u16, pub length: u16 }`
  - `TrackBuffer.weak_ranges: [Option<WeakBitRange>; 8]`
  - `TrackBuffer.weak_range_count: u8`
  - Pre-populated LFSR pseudo-random noise in place of `0x00` in G64 tracks.

- [ ] **Step 1: Write failing test for G64 weak range detection**

Create test verifying that loading a G64 track containing `0x00` runs populates `weak_ranges` in `TrackBuffer` and replaces `0x00` bytes with pseudo-random bytes masked with `& 0xFE`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pico1541-manager -- test_g64_weak`
Expected: FAIL

- [ ] **Step 3: Implement `WeakBitRange` in `crates/pico1541-common/src/track_buffer.rs`**

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WeakBitRange {
    pub offset: u16,
    pub length: u16,
}

pub const MAX_WEAK_RANGES: usize = 8;
```

Update `TrackBuffer` struct and `new()` to include `weak_ranges: [Option<WeakBitRange>; MAX_WEAK_RANGES]` and `weak_range_count: u8`.

- [ ] **Step 4: Implement weak range scanning in `crates/pico1541-manager/src/formats/g64.rs`**

In `G64Image::read_track()`:
Scan `dest[..track_len]` for runs of `0x00` bytes:
- For each run, record `WeakBitRange { offset, length }` up to `MAX_WEAK_RANGES`.
- Overwrite `0x00` bytes with deterministic LFSR pseudo-noise (masked with `& 0xFE` to guarantee no consecutive `0xFF` bytes can trigger false SYNC).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p pico1541-manager`
Expected: PASS

- [ ] **Step 6: Commit Task 1**

```bash
git add crates/pico1541-common crates/pico1541-manager
git commit -m "feat: Add WeakBitRange to TrackBuffer and G64 weak-bit scanning"
```

---

### Task 2: Fast LFSR PRNG & `SoftwareMagneticSurface` Parity

**Files:**
- Modify: `crates/pico1541-common/src/track_buffer.rs` (add `Lfsr16`)
- Modify: `crates/pico1541-common/src/magnetic_surface.rs`
- Test: `crates/pico1541-emulator/tests/weak_bits.rs`

**Interfaces:**
- Consumes: `TrackBuffer` with `weak_ranges`.
- Produces:
  - `Lfsr16::next_weak_byte(&mut self) -> u8`
  - `MagneticSurface::poll_revolution(&mut self)`
  - Multi-revolution variance in `SoftwareMagneticSurface`.

- [ ] **Step 1: Write failing test for multi-revolution variance**

In `crates/pico1541-emulator/tests/weak_bits.rs`:
- Load a track with a weak range.
- Read through revolution 1, recording bytes in the weak range.
- Read through revolution 2.
- Assert that bytes in the weak range differ between revolution 1 and revolution 2, while bytes outside the weak range remain strictly identical.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pico1541-emulator --test weak_bits`
Expected: FAIL

- [ ] **Step 3: Implement `Lfsr16` in `crates/pico1541-common/src/track_buffer.rs`**

Implement a fast 16-bit Galois LFSR:
```rust
#[derive(Copy, Clone, Debug)]
pub struct Lfsr16(pub u16);

impl Lfsr16 {
    pub const fn new(seed: u16) -> Self {
        Self(if seed == 0 { 0xACE1 } else { seed })
    }

    #[inline(always)]
    pub fn next_u8(&mut self) -> u8 {
        let bit = self.0 & 1;
        self.0 = (self.0 >> 1) ^ (if bit != 0 { 0xB400 } else { 0 });
        (self.0 & 0xFF) as u8
    }

    #[inline(always)]
    pub fn next_weak_byte(&mut self) -> u8 {
        // Mask with 0xFE to guarantee no 0xFF bytes, preventing false 10-bit SYNC
        self.next_u8() & 0xFE
    }
}
```

- [ ] **Step 4: Add `poll_revolution(&mut self)` to `MagneticSurface` & implement in `SoftwareMagneticSurface`**

In `SoftwareMagneticSurface`:
When `self.stream_idx == 0` (or wraps around `stream.length`), call internal helper to mutate the registered `weak_ranges` using `Lfsr16`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p pico1541-emulator --test weak_bits`
Expected: PASS

- [ ] **Step 6: Commit Task 2**

```bash
git add crates/pico1541-common crates/pico1541-emulator
git commit -m "feat: Implement Lfsr16 and SoftwareMagneticSurface weak-bit variance"
```

---

### Task 3: Hardware `PioMagneticSurface` Revolution Detection & SRAM Shadow Mutation

**Files:**
- Modify: `crates/pico1541-rp-common/src/emulation/magnetic_surface.rs`
- Modify: `crates/pico1541-rp-common/src/emulation/mod.rs`
- Modify: `crates/pico1541-emulator/src/drivemechanics.rs`

**Interfaces:**
- Consumes: RP2350 DMA Channel 0 `trans_count` register.
- Produces: Periodic in-SRAM mutation of weak-bit ranges on revolution boundaries without touching `try_read_ram_byte()`.

- [ ] **Step 1: Implement `poll_revolution` in `PioMagneticSurface`**

In `crates/pico1541-rp-common/src/emulation/magnetic_surface.rs`:
- Add `last_trans_count: u32`, `lfsr: Lfsr16`, `active_weak_ranges: [Option<WeakBitRange>; 8]`, `active_weak_count: u8`.
- In `set_stream_source(...)`: copy `weak_ranges` from `stream`.
- In `poll_revolution(&mut self)`:
  - If `self.active_weak_count == 0`, return immediately (1 cycle).
  - Read `remaining = pac::DMA.ch(0).trans_count().read().0`.
  - If `remaining > self.last_trans_count`:
    - DMA reloaded! Revolution completed!
    - Iterate over `active_weak_ranges`, write `self.lfsr.next_weak_byte()` into `CURRENT_TRACK_PTR` at `offset..offset + length`.
  - `self.last_trans_count = remaining`.

- [ ] **Step 2: Connect `poll_revolution` outside the 512-tick batch in `run_emulator`**

In `crates/pico1541-rp-common/src/emulation/mod.rs`:
In `run_emulator`, immediately after `for _ in 0..512 { emulator.tick(hw); }`:
Call `emulator.mechanics_mut().surface_mut().poll_revolution()`.
Confirm `try_read_ram_byte()` and the inner 512-tick loop remain 100% untouched.

- [ ] **Step 3: Verify build for RP2350 target**

Run: `cargo build --package pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Expected: PASS with 0 warnings.

- [ ] **Step 4: Commit Task 3**

```bash
git add crates/pico1541-rp-common crates/pico1541-emulator
git commit -m "feat(rp2350): Implement hardware revolution detection and SRAM weak-bit mutation"
```

---

### Task 4: Full Workspace Verification, Compiler Warnings & Release Build

**Files:**
- All workspace files.

- [ ] **Step 1: Run clippy across all workspace packages**

Run: `cargo clippy --workspace -- -D warnings`
Run: `cargo clippy -p nib2g64 -- -D warnings`
Expected: 0 warnings.

- [ ] **Step 2: Run all workspace tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Verify clean RP2350 release build**

Run: `cargo build --package pico1541-rp2350 --target thumbv8m.main-none-eabihf --release`
Expected: 0 errors, clean release binary.

- [ ] **Step 4: Final commit**

```bash
git commit --allow-empty -m "chore: Finalize Phase 2 weak-bit emulation implementation"
```
