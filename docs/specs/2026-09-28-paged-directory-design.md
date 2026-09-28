# Design Specification: Paged Track 18 Directory Navigation & Track-Mapped PRG Loading

**Date:** 2026-09-28  
**Branch:** `support-fb-tool`  
**Status:** Approved by User, Ready for Implementation Planning  

---

## 1. Executive Summary & Problem Statement

### The Problem
Previous attempts to support directory browsing for tools like CBM-FileBrowser (`fb.prg`) and multi-file listings relied on a dynamic 2-sector "treadmill" that attempted to stream directory entries by rewriting sectors 1 and 2 on Track 18 in real time using a timer. 
At 300 RPM (200 ms per revolution, ~9.5 ms per sector), software timers in Core 1 could not reliably stay synchronized with the physical read head. When fastloaders (such as Action Replay, Final Cartridge, or `fb`'s internal turbo loader) read Track 18, or when stock 1541 DOS performed interleave stepping, GCR data was frequently updated under the head, causing sync byte corruption, read timeouts, head bumps, and track seeking retries (18.0 &rarr; 18.1 &rarr; 18.0).

### The Solution
Instead of an unstable real-time sector treadmill, this specification introduces:
1. **Static Track 18 Directory Paging:** Track 18 is formatted as a 100% standard, static 1541 directory track with 19 physical sectors (Sector 0 BAM + Sectors 1..18 Directory Blocks = 144 entries max).
2. **Obvious Navigation Entries:** If entries exceed 144, the page injects `>>> MORE >>>` (`0x86` `DIR`) as the last entry, and `<<< PREV <<<` (`0x86` `DIR`) as the first entry on subsequent pages.
3. **SD2IEC `CD:` Interception:** Selecting `>>> MORE >>>` or `<<< PREV <<<` triggers `CD:>>> MORE >>>` / `CD:<<< PREV <<<`, advancing or rewinding the page and pulsing a simulated disk change (`force_disk_change`).
4. **Unique Track/Sector File Identification:** Each file entry on the active page is assigned an unambiguous, 1-to-1 unique `(first_track, first_sector)` pointer across Tracks 17..11. When 1541 DOS posts a read job to zero page (`$0000..$0005`), Core 0 immediately identifies the exact file selected and alerts Core 1.
5. **Multi-Track Downward PRG Streaming:** Core 1 opens the file and lays out complete tracks with standard interleave 10 stepping downwards ($T \to T-1 \to T-2 \dots$). The 20–30 ms physical head stepping latency allows Core 1 (which generates tracks in 1 ms) to prepare each track before the head settles.

---

## 2. Architecture & Subsystems

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Commodore 64 (C64)                              │
│                                                                        │
│   LOAD "$",8  or  CBM-FileBrowser (fb.prg)  or  Fastloader Cartridge   │
└─────────────────────────────────┬──────────────────────────────────────┘
                                  │ IEC Bus
                                  ▼
┌─────────────────────────────────┴──────────────────────────────────────┐
│                    RP2040 Core 0 (1541 Emulator)                       │
│                                                                        │
│  - 6502 CPU + VIAs (Cycle-exact 1 MHz)                                 │
│  - Standard 1541 ROM executes directory search & job queue             │
│  - IEC Snooper: Intercepts CD: commands on ATN falling edges           │
│  - SystemBus: Detects Read Job ($80) at $0000..$0005 to virtual (T, S) │
└─────────────────┬──────────────────────────────────┬───────────────────┘
                  │ CMD_CHANNEL (IPC)                │ DriveEvent / IPC
                  ▼                                  ▼
┌─────────────────┴──────────────────────────────────┴───────────────────┐
│                    RP2040 Core 1 (Manager & Storage)                   │
│                                                                        │
│  - TrackCacheTask & VirtualDiskImage                                   │
│  - Directory Pager: Holds current page index (0, 1, 2...)              │
│  - Track 18 Formatter: Static 19 sectors (BAM + 144 entries)           │
│  - Multi-Track PRG Formatter: Generates Tracks 17..1 with interleave 10│
│  - SD Card Driver: Reads folder entries and streams PRG bytes          │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Directory Paging on Track 18

### 3.1 Capacity & Entry Structure
* Standard Commodore 1541 Track 18 has 19 physical sectors:
  * **Sector 0:** BAM (Block Availability Map) + Disk Name + Disk ID.
  * **Sectors 1..18:** Directory blocks (18 sectors $\times$ 8 entries = **144 entries maximum**).
* **Page Layout:**
  * If `current_page > 0`: Slot 0 of Sector 1 is `<<< PREV <<<` (`file_type = 0x86` `DIR`, size = 0 blocks).
  * Slots $1..N$: Up to 142 real file/folder entries loaded from the SD card directory for this page.
  * If `has_more_entries`: The last entry on the page is `>>> MORE >>>` (`file_type = 0x86` `DIR`, size = 0 blocks).
* **Physical Sector Ordering:**
  Physical sectors headers around Track 18 are sequential $0, 1, 2, \dots, 18$.
  Directory sector chaining uses sequential pointers:
  $$\text{Sector 1} \to \text{Sector 2} \to \text{Sector 3} \dots \to \text{Sector } K \to (0, 0\text{xFF})$$
  Empty directory sectors beyond $K$ point to $(0, 0\text{xFF})$.
* **Static In-Memory Guarantee:**
  Once generated for a page, Track 18 is 100% static in memory. Core 1 never touches or rewrites Track 18 while it is being read. Fastloaders can cache the entire track or jump across sectors without timing jitter or sync corruption.

### 3.2 BAM Block (Track 18, Sector 0)
* Track 18 Sector 0 header points to `(18, 1)` (first directory sector).
* DOS Version: `0x41` ('A').
* BAM entries for Tracks 1..35 mark sectors allocated or free:
  * Track 18: 0 free blocks (`0x00, 0x00, 0x00, 0x00`).
  * Data tracks: Marked with standard free block counts and bitmaps.
* Disk Name: Derived from the current SD directory name (padded with `0xA0`).
* Disk ID: Pseudo-random 2-byte uppercase PETSCII ID (`0x41..0x5A`).

---

## 4. CD Command Interception & Navigation

### 4.1 Supported Commands
The ATN snooper captures commands sent to Channel 15 or hijacked `CD:` prefixes. `resolve_cd_path` handles the following actions:

| Command | Action | Resulting State |
| :--- | :--- | :--- |
| `CD:>>> MORE >>>` | Advance Page | `page += 1`, re-render Track 18, call `force_disk_change()` |
| `CD:...` | Advance Page (Alias) | `page += 1`, re-render Track 18, call `force_disk_change()` |
| `CD:<<< PREV <<<` | Previous Page | `page = page.saturating_sub(1)`, re-render Track 18, call `force_disk_change()` |
| `CD:..` or `CD:_` or `CD:\x1F` | Parent Directory | Go up to parent folder on SD card, `page = 0`, call `force_disk_change()` |
| `CD//` or bare `CD:` | Root Directory | Go to `/` on SD card, `page = 0`, call `force_disk_change()` |
| `CD:dirname` | Enter Subfolder | Enter `current_path/dirname`, `page = 0`, call `force_disk_change()` |
| `CD:image.d64` | Mount D64 Image | Mount `.d64` / `.g64` / `.nib`, switch drive mode to real disk image |

### 4.2 Simulated Physical Disk Change
Whenever a page change or directory change occurs:
1. Core 1 updates the directory entries for the new page.
2. Core 1 signals Core 0 via `force_disk_change()`.
3. Core 0 asserts zero-page `$001C` (disk change flag in 1541 DOS RAM).
4. The 1541 DOS invalidates its internal BAM and directory caches, ensuring that any subsequent `LOAD "$",8` or fastloader directory read fetches the fresh Track 18.

---

## 5. Unique Track/Sector File Identification

### 5.1 The 1-to-1 Mapping
Each of the 144 directory slots on the active page is assigned a unique, immutable starting `(first_track, first_sector)` pair:
* **Tracks 17 down to 11** are speed zone 3 ($21\text{ sectors/track}$, sectors $0..20$).
* Total capacity: $7 \text{ tracks} \times 21 \text{ sectors} = 147 \text{ slots}$ (ample for all 144 directory entries).

The mapping is defined by the bijection:
$$\text{first\_track} = 17 - \lfloor \text{slot\_index} / 21 \rfloor$$
$$\text{first\_sector} = \text{slot\_index} \pmod{21}$$

Conversely, when a `(track, sector)` is read:
$$\text{slot\_index} = (17 - \text{track}) \times 21 + \text{sector}$$

For example:
* Slot 0: Track 17, Sector 0
* Slot 1: Track 17, Sector 1
* ...
* Slot 20: Track 17, Sector 20
* Slot 21: Track 16, Sector 0
* ...
* Slot 143: Track 11, Sector 17

### 5.2 Zero-Page Job Queue Detection
When 1541 DOS or a fastloader searches Track 18 for a file, it reads the entry's `first_track` and `first_sector` from bytes 1 and 2 of the directory entry.
The 1541 DOS then sets up a read job in zero page:
* Job command: `$80` (READ) written to `$0000..$0005` (Buffers 0..4).
* Track register: `$0006 + \text{buf} \times 2` containing `first_track`.
* Sector register: `$0007 + \text{buf} \times 2` containing `first_sector`.

In `SystemBus::write`:
```rust
if addr <= 0x0005 && (val & 0x8F) == 0x80 && self.memory.read(0x001C) != 0 {
    let buf = addr as usize;
    let track = self.memory.read(0x06 + (buf * 2) as u16);
    let sector = self.memory.read(0x07 + (buf * 2) as u16);
    if (11..=17).contains(&track) && sector < 21 {
        let slot = (17 - track) * 21 + sector;
        self.host.select_virtual_file(slot);
    }
}
```
Core 0 sends an IPC event to Core 1 identifying `slot`. Core 1 immediately opens the corresponding file on the SD card.

---

## 6. Multi-Track Downward PRG Streaming

### 6.1 Downward Track Layout
Once a file is selected:
1. Core 1 opens the file handle for the selected file.
2. The file is streamed across tracks starting at `first_track` and stepping downwards ($T \to T-1 \to T-2 \dots$):
   * Each track holds up to 21 sectors ($21 \times 254 = 5,334$ bytes).
   * Sectors are laid out with standard **interleave 10**:
     $$\text{sector}_k = (k \times 10) \pmod{21}$$
   * The sector chain links each sector to the next within the track.
   * The final sector of Track $T$ links to `(track - 1, 0)`.
   * The final sector of the file has `next_track = 0` and `next_sector = bytes_remaining + 1`.

### 6.2 Step Latency & Head Timing
* When 1541 DOS or a fastloader finishes reading the last sector on Track $T$, it steps the physical head to Track $T-1$.
* The 1541 stepper motor rate in ROM takes **20–30 ms** to step one track and settle.
* Core 1 takes **~1 ms** to read 21 blocks from SD and format the entire GCR track.
* Therefore, Core 1 completes generating Track $T-1$ more than 15 ms before the head even settles on Track $T-1$. There is zero chance of missing a revolution or racing the head.

### 6.3 Capacity & Overflow Wrap
* Maximum possible C64 PRG file size: $64\text{ KB}$ ($65,536\text{ bytes} \approx 256\text{ sectors} \approx 12\text{ tracks}$).
* Tracks 17 down to 1 provide $17 \times 21 = 357\text{ sectors}$ ($90,678\text{ bytes}$).
* Any normal C64 PRG fits completely within Tracks 17..1.
* In the event that a file starts at a lower track (e.g. Track 11) and reaches Track 1 before EOF:
  The last sector of Track 1 links to `(Track 17, Sector 0)`, seamlessly ping-ponging back to Track 17.

---

## 7. Implementation Boundaries & Components

| Component | File | Responsibilities |
| :--- | :--- | :--- |
| **Paged Directory Source** | `pico1541-manager/src/storage.rs` | Stores up to 144 entries per page, handles paging index, injects `<<< PREV <<<` and `>>> MORE >>>`. |
| **Virtual Disk Formatter** | `pico1541-manager/src/formats/virtual_disk.rs` | Formats static Track 18 (19 sectors sequential) and streams PRG files downwards across Tracks 17..1 with interleave 10. |
| **CD Command Resolver** | `pico1541-manager/src/tasks/mount_manager/cd_handler.rs` | Resolves `CD:>>> MORE >>>`, `CD:<<< PREV <<<`, `CD:..`, `CD:dirname`, `CD:image.d64`. |
| **Mount Manager Task** | `pico1541-manager/src/tasks/mount_manager/mod.rs` | Handles page state transitions, triggers SD prefetch for 144 entries, issues `force_disk_change()`. |
| **Job Queue Snooper** | `pico1541-emulator/src/systembus.rs` | Monitors zero-page job queue writes to `$0000..$0005`, decodes `(track, sector)` to `slot`, and notifies Core 1. |
| **Host Bridge IPC** | `pico1541-common/src/interface.rs` & `bridge.rs` | Passes `select_virtual_file(slot)` from Core 0 to Core 1. |

---

## 8. Verification & Acceptance Criteria

1. **Static Track 18 Test:**
   Unit test verifying Track 18 formats all 19 sectors sequentially with valid BAM and sequential directory sector chaining ($1 \to 2 \to \dots \to K \to (0, 0\text{xFF})$).
2. **Page Navigation Test:**
   Unit test verifying that a directory with $> 144$ entries correctly splits across pages, injects `>>> MORE >>>` at the end of page 0, and injects `<<< PREV <<<` at the start of page 1.
3. **CD Command Resolver Test:**
   Unit test in `cd_handler.rs` verifying that `CD:>>> MORE >>>` and `CD:<<< PREV <<<` are recognized as page navigation commands.
4. **Byte-Exact Multi-Track PRG Test:**
   Unit test verifying that a PRG loaded via multi-track downward GCR decodes byte-for-byte identical to the original PRG file.
5. **No Compiler Warnings & Release Build:**
   `cargo test --workspace` passes cleanly with zero warnings, and `pico1541-rp2040` builds successfully in `--release` mode.
