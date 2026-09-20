# Design Specification: CBM-FileBrowser ("fb") & SD2IEC Command Interception

**Date:** 2026-09-20  
**Status:** Approved  
**Target:** Pico1541 (`pico1541-emulator`, `pico1541-manager`, `pico1541-rp-common`, `pico1541-rp2040`)

---

## 1. Overview & Objective

The objective is to support the CBM-FileBrowser tool (`fb` / `fb64`), commonly used with SD2IEC devices, allowing users to browse the SD card directory structure directly on a Commodore 64 and mount disk images (`.D64`, `.G64`, `.NIB`) without using the physical OLED/button UI.

This requires:
1. Intercepting SD2IEC DOS commands (`CD:...`, `CD//`, `CD:←`, `CD:..`) sent over the IEC command channel (secondary address 15) to Drive 8.
2. Nullifying the command in the 1541 DOS runtime to prevent `31, SYNTAX ERROR, 00, 00` and ensuring the error channel returns `00, OK, 00, 00`.
3. Routing the path/mount request to Core 0 (Manager), which updates the directory navigation or mounts the requested disk image into `TrackCacheTask`.
4. Ensuring that directories in `VirtualDiskImage` appear with the CBM file type `DIR` (`0x86`) so `fb` recognizes them as navigable folders.

---

## 2. IEC Electrical & Protocol Conventions

- **Inverter Interfacing:** The RP2040 IEC GPIOs are buffered via a 7405 open-collector inverter.
  - **Wired Bus LOW (Asserted)** $\rightarrow$ **GPIO pin HIGH (`true`)**.
  - **Wired Bus HIGH (Released)** $\rightarrow$ **GPIO pin LOW (`false`)**.
- **ATN Release Edge:** When the host completes an IEC transaction (e.g. after sending `UNLISTEN $3F`), the host releases the ATN line.
  - Wired bus transitions from LOW to HIGH.
  - GPIO `ATN_RX` transitions from **HIGH (`true`, Asserted)** to **LOW (`false`, Released)** (falling edge on GPIO).

---

## 3. Core 1 (Emulator) Interception Mechanism

Core 1 runs the 6502 CPU and VIA emulation cycle-by-cycle, polling physical GPIOs in `HardwareBridge::read_iec_port()`.

### 3.1 Trigger Condition
On each cycle, `HardwareBridge` monitors `ATN_RX`. When a falling edge is detected (GPIO transitions from HIGH to LOW, meaning the host released ATN):

1. **Fastloader / Custom Code Guard:**
   Check the 6502 program counter:
   - If `pc < 0xC000`, the drive CPU is executing custom code in drive RAM (e.g., fastloader read loops like Vorpal or RapidLok). **Skip immediately.**
2. **Command Buffer Inspection:**
   Read the 1541 DOS input command buffer at `$0200`:
   - Inspect `SYS_MEMORY[0x0200..0x0204]`.
   - Check for prefixes:
     - `b"CD:"` (change directory or mount image)
     - `b"CD/"` or `b"CD//"` (change to root or subdirectory)
     - `b"CD\x1F"` (PETSCII left arrow `←` / ASCII `_`, change to parent directory)
   - If no prefix matches, **return immediately**. No IPC, no interrupt, zero CPU overhead on Core 0.

### 3.2 Intercept Action & Error Suppression
When a matching prefix is identified:
1. Extract the full path/filename from `SYS_MEMORY[0x0200..0x0229]` up to carriage return `\r` (0x0D) or null terminator.
2. Send an `InternalCommand::Cd(heapless::String<64>)` message across cores via `CMD_CHANNEL`.
3. **Suppress 1541 DOS Syntax Error:**
   - Overwrite `SYS_MEMORY[0x0200] = 0` (or `\r`).
   - Clear the pending command flag: `SYS_MEMORY[0x0255] = 0` (`cmdwat = 0`).
   - Prime the 1541 DOS error message buffer (`errbuf` at `$02D5..0x02E2`) with `"00, OK,00,00\r"`.
   - Reset error status pointers: `SYS_MEMORY[0x006E] = 0`, `SYS_MEMORY[0x006F] = 0`.
4. When `fb` immediately queries channel 15 to check command success, 1541 DOS responds with `00, OK, 00, 00`.

---

## 4. Core 0 (Manager) Command & Navigation Logic

In `pico1541-manager`, `MountControllerTask` handles the new `InternalCommand::Cd`:

### 4.1 Path Parsing & Navigation Rules
- **Root Directory (`CD//` or `CD/`):**
  - Set `CURRENT_DIR` to `"/"`.
  - If a disk image (`.D64`, `.G64`, `.NIB`) was mounted, unmount it.
  - Mount `VirtualDiskImage` for the root SD card directory.
- **Parent Directory (`CD:←` or `CD:..`):**
  - If an image is currently mounted: unmount it and stay in the directory where the image resides.
  - If already in an SD folder: strip the trailing directory component (e.g., `/GAMES/RPG` $\rightarrow$ `/GAMES`). If at root `"/"`, do nothing.
- **Subdirectory (`CD:SUBDIR` or `CD/SUBDIR`):**
  - Verify that `SUBDIR` is a valid directory on the SD card.
  - Update `CURRENT_DIR` to `/path/to/SUBDIR`.
  - Instantiate `VirtualDiskImage` pointing to the new directory.
- **Disk Image (`CD:IMAGE.D64`, `CD:IMAGE.G64`, `CD:IMAGE.NIB`):**
  - Form absolute path: `<CURRENT_DIR>/IMAGE.D64`.
  - Signal `MountControllerTask` to perform the mount sequence (toggling write-protect to trigger physical disk swap detection in 1541 DOS).

---

## 5. VirtualDiskImage Directory Formatting

`fb` requires directories to appear with the `DIR` file type in the directory listing:
- In `pico1541-manager/src/formats/virtual_disk.rs`, when generating directory sectors (Track 18):
  - For directory entries representing folders: set file type byte to `0x80 | 0x06` (`0x86` = closed `DIR` file type) instead of `0x82` (`PRG`).
  - For disk images (`.D64`, `.G64`, `.NIB`) and files: retain `0x82` (`PRG`).

---

## 6. Verification & Test Plan

1. **Unit Tests (`pico1541-emulator`):**
   - Test `CD:` command snoop and error suppression: verify that when `OPEN 15,8,15,"CD:TEST"` is sent, `$0200` is intercepted, `cmdwat` is cleared, and reading channel 15 returns `00, OK, 00, 00`.
   - Test non-CD commands (e.g. `I`, `UJ`, `S:FILE`): verify they are untouched and handled by 1541 DOS normally.
   - Test fastloader guard: verify that when `pc < 0xC000`, rapid ATN transitions do not trigger any command checks.
2. **Manager Navigation Tests (`pico1541-manager`):**
   - Verify path resolution for `CD//`, `CD:..`, `CD:←`, `CD:GAMES`, `CD:GAME.D64`.
   - Verify `VirtualDiskImage` generates correct `DIR` headers for directories.
3. **Hardware / Release Build:**
   - Verify `cargo test --workspace` passes cleanly.
   - Verify `pico1541-rp2040` builds in `--release` mode.
