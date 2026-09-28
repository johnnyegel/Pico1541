# Implementation Plan: Paged Track 18 Directory Navigation & Track-Mapped PRG Loading

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement static Track 18 directory paging with `>>> MORE >>>` / `<<< PREV <<<` navigation, SD2IEC `CD:` command interception, 1541 job-queue track/sector file identification, and multi-track downward PRG streaming.

**Architecture:** 
1. `PagedDirectorySource` maintains a page index and formats up to 142 entries per page, inserting `<<< PREV <<<` at the start of pages > 0 and `>>> MORE >>>` at the end if more entries exist on SD.
2. `VirtualDiskImage` formats Track 18 statically with sequential physical sectors 0..18 and standard BAM.
3. `cd_handler` recognizes `CD:>>> MORE >>>` and `CD:<<< PREV <<<`, incrementing or decrementing the page index and pulsing `force_disk_change()`.
4. `SystemBus::write` snoops 1541 zero-page job queue writes to `$0000..$0005` to map `(first_track, first_sector)` to the selected file index and notifies Core 1 via IPC.
5. `VirtualDiskImage` streams PRG bytes downwards across Tracks 17..1 with interleave 10.

**Architecture Diagram:**

```mermaid
graph TD
    subgraph "Core 0 (1541 Emulator)"
        ROM[1541 DOS ROM] -->|Directory Search| T18[Track 18 Static GCR]
        ROM -->|Job $80 (Track, Sector)| BUS[SystemBus::write $0000..$0005]
        BUS -->|select_virtual_file(slot)| BRIDGE[HardwareBridge]
        IEC[IEC Snooper] -->|CD:>>> MORE >>> / CD:<<< PREV <<<| BRIDGE
    end

    BRIDGE -->|CMD_CHANNEL (IPC)| MGR[MountControllerTask (Core 1)]

    subgraph "Core 1 (Manager & Storage)"
        MGR -->|Page Nav / force_disk_change| PAGER[PagedDirectorySource]
        PAGER -->|144 Static Entries| VDISK[VirtualDiskImage]
        MGR -->|Select File| SD[SD Card Storage]
        SD -->|PRG Stream| VDISK
        VDISK -->|Track 18 Static / Tracks 17..1 Downward| GCR[Accurate GCR Tracks]
    end
```

**Tech Stack:** Rust (`no_std`), Embassy, RP2040 Dual-Core IPC, GCR Encoding, Commodore 1541 Bus/DOS Architecture.

## Global Constraints
* All firmware crates (`pico1541-emulator`, `pico1541-manager`, `pico1541-common`, `pico1541-rp2040`) must remain `no_std`.
* Branch: `support-fb-tool` (do not merge or push to GitHub).
* Dual-Core timing invariants must be strictly preserved.
* Zero compiler warnings; all unit tests must pass; RP2040 must build in `--release` mode.

---

### Task 1: Paged Directory Source & Navigation Entries

**Files:**
- Modify: `crates/pico1541-manager/src/storage.rs:241-320`
- Test: `crates/pico1541-manager/tests/formats.rs`

**Interfaces:**
- Consumes: `DirectoryEntry::from_file_info`, `mangle_filename`
- Produces: `PagedDirectorySource::set_page(page: usize)`, `PagedDirectorySource::current_page(&self) -> usize`, unique `(first_track, first_sector)` assignment per entry.

- [ ] **Step 1: Write the failing unit tests for directory pagination and navigation entries**
In `crates/pico1541-manager/tests/formats.rs`:
```rust
#[test]
fn test_paged_directory_navigation_entries() {
    use pico1541_manager::storage::{DirectoryEntry, PagedDirectorySource, DirectorySource};

    let mut source = PagedDirectorySource::<144>::new(b"TEST DIR");
    source.set_total_count(200); // More than 1 page

    // Page 0: No previous entry, ends with ">>> MORE >>>"
    source.set_page(0);
    let mut page0_entries = Vec::new();
    while let Some(entry) = source.next_entry() {
        page0_entries.push(entry);
    }
    assert_eq!(page0_entries.len(), 144);
    assert_ne!(&page0_entries[0].filename[..12], b"<<< PREV <<<");
    assert_eq!(&page0_entries[143].filename[..12], b">>> MORE >>>");
    assert_eq!(page0_entries[143].file_type, 0x86); // DIR

    // Page 1: Starts with "<<< PREV <<<", ends with real entries
    source.set_page(1);
    let mut page1_entries = Vec::new();
    while let Some(entry) = source.next_entry() {
        page1_entries.push(entry);
    }
    assert_eq!(&page1_entries[0].filename[..12], b"<<< PREV <<<");
    assert_eq!(page1_entries[0].file_type, 0x86); // DIR
}

#[test]
fn test_directory_entry_unique_track_sector_mapping() {
    use pico1541_manager::storage::slot_to_track_sector;

    // Slot 0 -> Track 17, Sector 0
    assert_eq!(slot_to_track_sector(0), (17, 0));
    // Slot 20 -> Track 17, Sector 20
    assert_eq!(slot_to_track_sector(20), (17, 20));
    // Slot 21 -> Track 16, Sector 0
    assert_eq!(slot_to_track_sector(21), (16, 0));
    // Slot 143 -> Track 11, Sector 17
    assert_eq!(slot_to_track_sector(143), (11, 17));
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p pico1541-manager --test formats test_paged_directory_navigation_entries`
Expected: FAIL (methods missing).

- [ ] **Step 3: Implement `slot_to_track_sector` and pagination in `storage.rs`**
Add helper in `storage.rs`:
```rust
#[inline(always)]
pub fn slot_to_track_sector(slot: usize) -> (u8, u8) {
    let track = 17 - (slot / 21) as u8;
    let sector = (slot % 21) as u8;
    (track, sector)
}

#[inline(always)]
pub fn track_sector_to_slot(track: u8, sector: u8) -> Option<usize> {
    if (11..=17).contains(&track) && sector < 21 {
        Some((17 - track) as usize * 21 + sector as usize)
    } else {
        None
    }
}
```
Update `PagedDirectorySource`:
- Add `page: usize`.
- Add `pub fn set_page(&mut self, page: usize)`.
- When iterating via `next_entry()`:
  - If `self.page > 0` and index == 0: Return `<<< PREV <<<` entry (`0x86` DIR, 0 blocks).
  - If `has_more` and index == last_slot: Return `>>> MORE >>>` entry (`0x86` DIR, 0 blocks).
  - For real entries, populate `entry.first_track` and `entry.first_sector` using `slot_to_track_sector(slot_index)`.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p pico1541-manager --test formats test_paged_directory_navigation_entries`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/pico1541-manager/src/storage.rs crates/pico1541-manager/tests/formats.rs
git commit -m "feat(manager): paged directory source with navigation entries and track/sector mapping"
```

---

### Task 2: Static Sequential Track 18 Formatter

**Files:**
- Modify: `crates/pico1541-manager/src/formats/virtual_disk.rs:460-580`
- Test: `crates/pico1541-manager/tests/formats.rs`

**Interfaces:**
- Consumes: `DirectorySource`, `sectors_per_track`
- Produces: 100% static Track 18 GCR buffer with sectors 0..18 sequential.

- [ ] **Step 1: Write unit test for complete static Track 18 layout**
In `crates/pico1541-manager/tests/formats.rs`:
```rust
#[test]
fn test_virtual_disk_static_track_18_all_sectors() {
    use pico1541_manager::formats::VirtualDiskImage;
    use pico1541_manager::storage::VectorDirectorySource;
    use pico1541_manager::DiskImage;
    use embedded_io_adapters::futures_03::FromFutures;

    // Create 30 entries (fills Sector 1, 2, 3, and part of 4)
    let mut entries = heapless::Vec::<pico1541_manager::storage::DirectoryEntry, 144>::new();
    for i in 0..30 {
        let name = format!("FILE{}", i);
        let _ = entries.push(pico1541_manager::storage::DirectoryEntry::from_file_info(name.as_bytes(), false, 1024));
    }
    let source = VectorDirectorySource::new(b"TEST DISK", entries);
    let mut vdisk = VirtualDiskImage::new(source, b"TEST DISK");

    let mut dest = vec![0u8; 10000];
    let (layout, len, _) = futures::executor::block_on(vdisk.read_track(34, &mut dest)).unwrap();
    assert_eq!(layout, pico1541_common::track_buffer::TrackLayout::Accurate);

    // Verify all 19 physical sectors (0..18) are present
    // and directory sectors 1..4 chain sequentially
}
```

- [ ] **Step 2: Run test to verify current behavior**
Run: `cargo test -p pico1541-manager --test formats test_virtual_disk_static_track_18_all_sectors`

- [ ] **Step 3: Update `read_track` for Track 18 in `virtual_disk.rs`**
- Populate Sector 0 with BAM (free counts, disk name, ID).
- Populate Sectors 1..18 sequentially from `self.source`.
  - For each sector $S \in 1..18$: read up to 8 entries.
  - If entries remain after sector $S$: `next_track = 18`, `next_sector = S + 1`.
  - If no more entries: `next_track = 0`, `next_sector = 0xFF`.
  - Empty sectors beyond the last directory sector are formatted with `(0, 0xFF)`.
- Physical sector order on disk: sequential $0, 1, 2, \dots, 18$.
- Remove all treadmill sector index references (`self.sector1_idx`, `self.sector2_idx`).

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p pico1541-manager --test formats test_virtual_disk_static_track_18_all_sectors`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/pico1541-manager/src/formats/virtual_disk.rs crates/pico1541-manager/tests/formats.rs
git commit -m "feat(manager): static 19-sector sequential track 18 formatter without treadmill"
```

---

### Task 3: CD Command Interception for Page Navigation

**Files:**
- Modify: `crates/pico1541-manager/src/tasks/mount_manager/cd_handler.rs`
- Modify: `crates/pico1541-manager/src/tasks/mount_manager/mod.rs`
- Test: `crates/pico1541-manager/tests/formats.rs`

**Interfaces:**
- Consumes: `InternalCommand::Cd(cd_str)`
- Produces: `CdAction` enum distinguishing `ChangeDir(path)`, `NextPage`, `PrevPage`, `MountImage(path)`.

- [ ] **Step 1: Write unit tests for `resolve_cd_action`**
In `crates/pico1541-manager/tests/formats.rs`:
```rust
#[test]
fn test_resolve_cd_navigation_commands() {
    use pico1541_manager::tasks::mount_manager::cd_handler::{resolve_cd_action, CdAction};

    assert_eq!(resolve_cd_action("/", "CD:>>> MORE >>>"), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:..."), Some(CdAction::NextPage));
    assert_eq!(resolve_cd_action("/", "CD:<<< PREV <<<"), Some(CdAction::PrevPage));
    assert_eq!(resolve_cd_action("/GAMES", "CD:.."), Some(CdAction::ChangeDir("/".into())));
    assert_eq!(resolve_cd_action("/", "CD:GAMES"), Some(CdAction::ChangeDir("/GAMES".into())));
    assert_eq!(resolve_cd_action("/", "CD:GAME.D64"), Some(CdAction::MountImage("/GAME.D64".into())));
}
```

- [ ] **Step 2: Run test to verify it fails**
Run: `cargo test -p pico1541-manager --test formats test_resolve_cd_navigation_commands`
Expected: FAIL.

- [ ] **Step 3: Implement `CdAction` and `resolve_cd_action` in `cd_handler.rs`**
Update `cd_handler.rs` to return `CdAction`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CdAction {
    NextPage,
    PrevPage,
    ChangeDir(heapless::String<128>),
    MountImage(heapless::String<128>),
}
```
Check for `>>> MORE >>>` / `...` &rarr; `NextPage`.
Check for `<<< PREV <<<` &rarr; `PrevPage`.
Check for `.d64`/`.g64`/`.nib` &rarr; `MountImage`.
Otherwise &rarr; `ChangeDir`.

Update `MountControllerTask::run()` in `mount_manager/mod.rs`:
- On `NextPage`: `current_page += 1`, update `PagedDirectorySource`, call `self.track_handle.force_disk_change()`.
- On `PrevPage`: `current_page = current_page.saturating_sub(1)`, update `PagedDirectorySource`, call `self.track_handle.force_disk_change()`.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p pico1541-manager --test formats test_resolve_cd_navigation_commands`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/pico1541-manager/src/tasks/mount_manager/cd_handler.rs crates/pico1541-manager/src/tasks/mount_manager/mod.rs crates/pico1541-manager/tests/formats.rs
git commit -m "feat(manager): handle CD page navigation commands and simulated disk swap"
```

---

### Task 4: Job Queue Snooping & Virtual File Selection IPC

**Files:**
- Modify: `crates/pico1541-emulator/src/systembus.rs`
- Modify: `crates/pico1541-common/src/interface.rs`
- Modify: `crates/pico1541-rp-common/src/emulation/bridge.rs`
- Modify: `crates/pico1541-manager/src/lib.rs`
- Test: `crates/pico1541-emulator/tests/common/mod.rs`

**Interfaces:**
- Consumes: Zero-page `$0000..$0005` writes in `SystemBus::write`
- Produces: `HostInterface::select_virtual_file(slot: usize)`, `InternalCommand::SelectVirtualFile(usize)`.

- [ ] **Step 1: Write test in emulator test suite**
In `crates/pico1541-emulator/tests/virtual_disk.rs`:
Test that when 1541 zero page receives a job `$80` for Track 17 Sector 2, `select_virtual_file(2)` is invoked on `TestBridge`.

- [ ] **Step 2: Add `select_virtual_file` to `HostInterface` and `InternalCommand`**
In `crates/pico1541-common/src/interface.rs`:
```rust
fn select_virtual_file(&mut self, _slot: usize) {}
```
In `crates/pico1541-manager/src/lib.rs`:
```rust
SelectVirtualFile(usize),
```
In `crates/pico1541-rp-common/src/emulation/bridge.rs`:
Implement `select_virtual_file` forwarding to `CMD_CHANNEL`.

- [ ] **Step 3: Add job queue snooping in `SystemBus::write`**
In `crates/pico1541-emulator/src/systembus.rs`:
```rust
if addr <= 0x0005 && (val & 0x8F) == 0x80 && self.memory.read(0x001C) != 0 {
    let buf = addr as usize;
    let track = self.memory.read(0x06 + (buf * 2) as u16);
    let sector = self.memory.read(0x07 + (buf * 2) as u16);
    if (11..=17).contains(&track) && sector < 21 {
        let slot = (17 - track) as usize * 21 + sector as usize;
        self.host.select_virtual_file(slot);
    }
}
```

- [ ] **Step 4: Run tests to verify it compiles and passes**
Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/pico1541-emulator/src/systembus.rs crates/pico1541-common/src/interface.rs crates/pico1541-rp-common/src/emulation/bridge.rs crates/pico1541-manager/src/lib.rs
git commit -m "feat(ipc): snoop 1541 job queue to select virtual file by track and sector"
```

---

### Task 5: Multi-Track Downward PRG Streaming

**Files:**
- Modify: `crates/pico1541-manager/src/formats/virtual_disk.rs:300-458`
- Test: `crates/pico1541-manager/tests/formats.rs`

**Interfaces:**
- Consumes: `VirtualDiskImage::file_storage`, `first_track`
- Produces: Seamless multi-track downward GCR tracks with interleave 10 and EOF handling.

- [ ] **Step 1: Write byte-exact multi-track PRG decode test**
Verify in `crates/pico1541-manager/tests/formats.rs` that a PRG spanning 3 tracks (e.g. Tracks 17, 16, 15) decodes byte-for-byte across track boundaries.

- [ ] **Step 2: Run test to verify current behavior**
Run: `cargo test -p pico1541-manager --test formats test_virtual_disk_prg_byte_exact`

- [ ] **Step 3: Update `read_track` PRG streaming in `virtual_disk.rs`**
- Calculate track byte offset relative to the file's starting track:
  $$\text{track\_offset} = (T_{\text{start}} - \text{track}) \times 21 \times 254$$
- Sectors formatted with interleave 10: `(step * 10) % 21`.
- If track reaches Track 1 and more file bytes remain, next sector points to `(Track 17, Sector 0)` (wrap/ping-pong).
- Proper end-of-file marking: `next_track = 0`, `next_sector = bytes_read + 1`.

- [ ] **Step 4: Run test to verify it passes**
Run: `cargo test -p pico1541-manager --test formats`
Expected: PASS.

- [ ] **Step 5: Commit**
```bash
git add crates/pico1541-manager/src/formats/virtual_disk.rs crates/pico1541-manager/tests/formats.rs
git commit -m "feat(manager): multi-track downward PRG streaming with interleave 10 and track wrap"
```

---

### Task 6: Workspace Build, Release Verification & Final Cleanup

**Files:**
- Review: `crates/pico1541-manager`, `crates/pico1541-emulator`, `crates/pico1541-rp2040`

- [ ] **Step 1: Run full workspace test suite**
Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 2: Build RP2040 firmware in release mode**
Run: `cargo build --package pico1541-rp2040 --release --target thumbv6m-none-eabi`
Expected: Compiles with 0 errors and 0 warnings.

- [ ] **Step 3: Remove unused treadmill functions or dead code**
Clean up any remaining spinup/treadmill dead code in `virtual_disk.rs` and verify clean git status.

- [ ] **Step 4: Commit**
```bash
git add .
git commit -m "chore(cleanup): remove legacy treadmill code and verify clean release build"
```
