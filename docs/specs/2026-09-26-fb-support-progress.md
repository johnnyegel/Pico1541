# fb Tool Support — Progress Ledger

## Branch: `support-fb-tool`

## Status Summary

| Task | Description | Status | Commit |
|------|-------------|--------|--------|
| T1 | Directory entries: `0x86` for dirs, `0x82` for files | ✅ Done | `11c7cc1` |
| T2 | `InternalCommand::Cd` + `HostInterface::intercept_cd_command` | ✅ Done | `f2853c6` |
| T3 | Emulator ATN falling-edge CD: interception + 00,OK status | ✅ Done | `8858a01` |
| T4 | `MountControllerTask` Cd handling + `resolve_cd_path` | ✅ Done | `366d194` |
| **A** | **Fix empty directory listing (initial sync prefetch)** | ✅ Done | `b2c3b1a` |
| B | Root listing: `[INTERNAL]` and `[SD CARD]` entries | ⏳ Planned | — |
| C | File/image loading via track/sector encoding | ⏳ Planned | — |

---

## Plan A: Fix Empty Directory Listing

### Root Cause
`mount_sequence()` creates a `VirtualDiskImage` with an empty `PagedDirectorySource`, then
immediately calls `handle_track_event()` which generates Track 18 GCR with 0 entries.
The timer-driven prefetcher fires 20ms later and populates entries — too late, the drive
has already streamed the empty directory to the C64.

### Fix
After creating the `VirtualDiskImage` and before `handle_track_event()`, do a **synchronous
blocking fetch** of the first page (16 entries) from `list_dir_range()` and populate block 0
of the `PagedDirectorySource`. The 400ms disk-swap delay window is sufficient for this.

### Files Changed
- `crates/pico1541-manager/src/tasks/mount_manager/mod.rs`
  — Add initial fetch block in `mount_sequence()` for non-image paths.

### Tests
- `cargo test -p pico1541-manager` (no new test needed; existing tests verify the existing behavior)
- Manual: connect C64, run `LOAD"$",8` — expect visible file listing.

---

## Plan B: Root Listing

### Description
When path is `/`, inject two fixed entries into a `VectorDirectorySource<2>`:
- `[INTERNAL]` (DIR type 0x86, first_track=1, first_sector=0)
- `[SD CARD]`  (DIR type 0x86, first_track=1, first_sector=1)

Navigation to these is handled by the UI, not the C64.

---

## Plan C: File/Image Mounting via Track/Sector Encoding

### Description
- Encode `file_index` into directory entry `first_track`/`first_sector`:
  - `first_track = (file_index / 21) + 2`
  - `first_sector = file_index % 21`
- When `read_track()` is called for a "virtual file track" (track 2–17, 19–35),
  serve file data from SD card (open file at that index, stream as GCR sectors).
- If the file is `.D64`/`.G64`/`.NIB`, trigger `InternalCommand::Mount` instead of streaming.
- Works with fastloaders because T/S is resolved from directory by ROM *before* any
  fastloader code uploads to drive RAM.
