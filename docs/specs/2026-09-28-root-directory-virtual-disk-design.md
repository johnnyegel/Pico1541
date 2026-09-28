# Design Specification: Root Directory Virtual Disk & Unified Location Architecture

**Date:** 2026-09-28  
**Branch:** `support-fb-tool`  
**Status:** Approved for Implementation

---

## 1. Overview & Goals

In the Pico1541 architecture, the emulated 1541 drive (Core 0 / Core 1 manager) and the physical OLED UI have previously maintained disjoint path states (`CURRENT_DIR` in the UI vs `MOUNTED_DIR` in the mount controller). This separation resulted in discrepancies when SD2IEC `CD:` commands were intercepted from tools like `fb64` (CBM-FileBrowser): the drive changed directories, but the OLED display remained stuck on the previous directory. Furthermore, bare `CD:` navigation from tools expecting a top-level root switched directly to the raw SD card rather than presenting the unified storage environment.

This design introduces:
1. **Unified Location Architecture (`DriveLocation`):** A single source of truth for the drive's current location and mount state, eliminating disjoint path variables and redundant synchronization logic.
2. **Root Directory Virtual Disk:** When located at the top-level `Root`, Track 18 generates a standard 1541 directory presenting two virtual entries:
   - `INTERNAL` (`0x86` `DIR`) &rarr; navigates into the internal flash storage user files (`/files`).
   - `SD CARD` (`0x86` `DIR`, labeled with FAT volume name if available) &rarr; navigates into the SD card root (`/`).
3. **Reactive UI & Drive Alignment:** The OLED user interface and the 1541 drive emulator become pure reactive views of `CURRENT_LOCATION`. Changes initiated by device buttons or C64 DOS commands modify this single state, immediately updating both the emulated disk surface and the OLED screen.

---

## 2. Architecture & Data Structures

### 2.1 The `DriveLocation` Enum

Located in `crates/pico1541-common/src/location.rs` (or `pico1541-manager`):

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DriveLocation {
    /// Top-level root container.
    /// Track 18 generates two directory entries: "INTERNAL" and "SD CARD".
    Root,

    /// Virtual directory listing within either Internal flash (/files) or SD card.
    VirtualDirectory {
        path: heapless::String<128>,
        use_internal: bool,
    },

    /// A mounted disk image file (.D64, .G64, .NIB).
    DiskImage {
        path: heapless::String<128>,
        use_internal: bool,
    },
}
```

### 2.2 Single Source of Truth

The separate `CURRENT_DIR` and `MOUNTED_DIR` static variables across `pico1541-rp2040` and `pico1541-rp2350` are consolidated into a single shared mutex:

```rust
pub static CURRENT_LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
    Mutex::new(DriveLocation::Root);
```

To allow reactive awakening of the UI without continuous polling, an atomic notification or signal is updated alongside location changes:
```rust
pub static LOCATION_CHANGED: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();
```

---

## 3. Root Directory Virtual Disk

### 3.1 Track 18 Formatter Layout
When `CURRENT_LOCATION == DriveLocation::Root`:
- **Disk Header (BAM Sector 0):**
  - Disk Name: `b"STORAGE         "`
  - Disk ID: `b"01"`
  - DOS Version / Format: `b"2A"`
  - BAM Sector points to Track 18 Sector 1.
- **Directory Sector (Track 18 Sector 1):**
  - Forward Pointer: `(0, 0xFF)` (last sector, 2 entries total).
  - **Slot 0 (`INTERNAL`):**
    - File Type: `0x86` (`DIR`)
    - Target: Track 17, Sector 0 (maps to Slot 0 for job queue selection)
    - Filename: `INTERNAL` padded with PETSCII `0xA0`
  - **Slot 1 (`SD CARD`):**
    - File Type: `0x86` (`DIR`)
    - Target: Track 17, Sector 1 (maps to Slot 1 for job queue selection)
    - Filename: Volume label of SD card (e.g. `SD CARD` or FAT volume label, max 16 chars) padded with PETSCII `0xA0`
  - Slots 2..7: Unused (`0x00`).

### 3.2 Directory Source
A dedicated `RootDirectorySource` or a 2-entry `VectorDirectorySource<2>` is used:
```rust
let mut entries = heapless::Vec::<DirectoryEntry, 2>::new();
let _ = entries.push(DirectoryEntry::new_dir(b"INTERNAL", 17, 0));
let sd_label = sd_volume_label.unwrap_or(b"SD CARD         ");
let _ = entries.push(DirectoryEntry::new_dir(&sd_label, 17, 1));
let source = VectorDirectorySource::new(b"STORAGE", entries);
let vdisk = VirtualDiskImage::new(source, b"STORAGE");
```

On the C64, running `LOAD "$",8` at Root produces:
```text
0 "STORAGE"         01 2A
0    "INTERNAL"         DIR
0    "SD CARD"          DIR
664 BLOCKS FREE.
```

---

## 4. Navigation & State Transitions

### 4.1 State Transition Matrix

| Current Location | Incoming Command / Action | Target Location | Actions Triggered |
| :--- | :--- | :--- | :--- |
| **`Root`** | `CD:INTERNAL` or `CD/INTERNAL` or Select Slot 0 | `VirtualDirectory { path: "/files", use_internal: true }` | `force_disk_change()`, load `/files` Track 18 |
| **`Root`** | `CD:SD CARD` or `CD:SD` or Select Slot 1 | `VirtualDirectory { path: "/", use_internal: false }` | `force_disk_change()`, load SD `/` Track 18 |
| **`VirtualDirectory`** | `CD:..` or `CD:_` or `CD:<-` at top level (`/` or `/files`) | `Root` | `force_disk_change()`, load Root Track 18 |
| **`VirtualDirectory`** | `CD:..` in subfolder `/A/B` | `VirtualDirectory { path: "/A", ... }` | `force_disk_change()`, load parent Track 18 |
| **`VirtualDirectory`** | `CD:SUBDIR` | `VirtualDirectory { path: "/.../SUBDIR", ... }` | `force_disk_change()`, load subfolder Track 18 |
| **`VirtualDirectory`** | `CD:` or `CD//` | `Root` | `force_disk_change()`, load Root Track 18 |
| **`VirtualDirectory`** | `CD:IMAGE.D64` or Select Image | `DiskImage { path: "/.../IMAGE.D64", ... }` | `force_disk_change()`, mount `.d64` / `.g64` / `.nib` |
| **`DiskImage`** | `CD:..` or `CD:` or `CD<-` | `VirtualDirectory` of the containing folder | `force_disk_change()`, restore virtual folder |

### 4.2 Page Navigation within a Virtual Directory
When browsing a directory with > 142 items:
- `CD:>>> MORE >>>` or `CD:...` advances `current_page`.
- `CD:<<< PREV <<<` decrements `current_page`.
- Location remains unchanged; `PAGE_CHANGED` triggers `force_disk_change()` and directory reload for that page.

---

## 5. Reactive UI Integration

### 5.1 Elimination of Redundant UI State
- In `MainMenuTask`, remove separate `sd_path`, `internal_path`, and `viewed_dir`.
- The OLED display renders whatever location is set in `CURRENT_LOCATION`.

### 5.2 Event Loop Awakening
In `crates/pico1541-manager/src/tasks/menu.rs`:
The menu wait loop selects across physical button inputs (`action_rx`) and location update signals:
```rust
match select(action_rx.receive(), LOCATION_CHANGED.wait()).await {
    Either::First(action) => process_button(action),
    Either::Second(_) => return MenuEvent::LocationUpdated,
}
```
When `MenuEvent::LocationUpdated` is received:
- `run_file_browser_menu` / `MainMenuTask` re-reads `CURRENT_LOCATION`.
- If the location switched between `Root`, `VirtualDirectory`, or `DiskImage`, the UI transitions to the corresponding page state.
- Cursor and scroll offset are reset to 0, and the OLED display renders the updated directory.

---

## 6. Testing & Verification

1. **Unit Tests:**
   - Root virtual disk formatting: verify Track 18 Sector 0 (BAM) and Sector 1 contain `INTERNAL` and `SD CARD` directory entries with types `0x86`.
   - `resolve_cd_action`: verify `CD:` and `CD//` transition to `Root`, while `CD:INTERNAL` and `CD:SD` enter the respective containers.
   - `DriveLocation` state machine transitions and parent directory navigation.
2. **Integration & Build Verification:**
   - `cargo test --workspace` on host.
   - `cargo build --package pico1541-rp2040 --release --target thumbv6m-none-eabi`.
   - `cargo build --package pico1541-rp2350 --release --target thumbv8m.main-none-eabihf`.
