# Pico1541

A cycle-accurate Commodore 1541 disk drive emulator running on the Raspberry Pi Pico 2 (RP2350).

Unlike high-level IEC drive emulators (such as standard SD2IEC devices), **Pico1541** provides true hardware-level emulation. It models the MOS 6502 CPU and dual MOS 6522 VIAs in sub-cycle lockstep at 1 MHz, alongside a custom PIO-based magnetic track surface. As a result, Pico1541 is compatible with custom fastloaders, custom DOS routines, and sophisticated multi-stage copy protection schemes that execute code directly inside the drive.

---

## Table of Contents

- [Features](#features)
- [Emulation Status & Known Limitations](#emulation-status--known-limitations)
- [Architecture](#architecture)
- [Hardware & Pinout](#hardware--pinout)
- [ROM Installation & Legal Notice](#rom-installation--legal-notice)
  - [Compile-Time Embedding (Flash LittleFS)](#compile-time-embedding-flash-littlefs)
  - [Switching ROMs at Runtime (OLED Menu)](#switching-roms-at-runtime-oled-menu)
- [Prerequisites](#prerequisites)
- [Building & Flashing](#building--flashing)
  - [Building the Release Binary](#building-the-release-binary)
  - [Flashing via USB (BOOTSEL Mode)](#flashing-via-usb-bootsel-mode)
  - [Flashing via SWD Debugger](#flashing-via-swd-debugger)
- [Usage & Operation](#usage--operation)
  - [IEC Cable Connection](#iec-cable-connection)
  - [OLED Menu Navigation](#oled-menu-navigation)
  - [C64 Commands & Browser](#c64-commands--browser)
- [Repository Structure](#repository-structure)
- [Licensing](#licensing)

---

## Features

- **Cycle-Accurate Emulation Core:** Driven by `racc6502v4`, a dedicated micro-instruction 6502 CPU core executing in sub-cycle lockstep with dual 6522 VIAs at 1 MHz.
- **Fastloader & Protection Compatibility:** Compatible with popular fastloaders and custom drive code (Action Replay, Final Cartridge III, Epyx Fastload, Vorpal, V-MAX!, RapidLok, and custom track loaders).
- **Supported Disk Formats:**
  - **`.D64`**: Standard 35-track and 40-track Commodore disk images (dynamic on-the-fly GCR encoding/decoding).
  - **`.G64`**: Raw GCR track bitstreams with custom sector interleave and variable speed zones.
  - **`.PRG`**: Mount standalone PRG files as virtual single-file disks on the fly.
- **Dual-Core Architecture (RP2350):**
  - **Core 1 (Emulator):** Dedicated entirely to the cycle-exact 6502 CPU, 6522 VIAs, and PIO magnetic surface streaming.
  - **Core 0 (Manager):** Runs asynchronous Embassy tasks managing the SD card, flash filesystem, OLED display, navigation buttons, and virtual directory synthesis.
- **Hardware PIO Magnetic Surface:** Utilizes RP2350 Programmable I/O (PIO) state machines and two-channel ping-pong DMA chaining for zero-CPU-overhead track data streaming to the emulated read head.
- **Storage Options:**
  - **MicroSD Card (FAT32):** Mount disk images directly from folders on your SD card.
  - **Internal Flash (LittleFS):** Built-in flash filesystem for bundled utilities, settings, and ROM storage.
- **On-Device Interface:**
  - 128x64 I2C SSD1306 OLED display.
  - 4-button tactile keypad (Up, Down, Select/Enter, Back/Exit).
  - RGB Status LED and Dual-color Drive Activity LED.
  - Piezo speaker providing authentic mechanical head-stepping acoustics and seek sounds.
- **SD2IEC Navigation & File Browser:** Intercepts `CD:` commands on IEC channel 15 to allow navigating SD card folders and disk images using standard Commodore 64 file browsers such as `fb64`.
- **Configurable Drive ID:** Switch drive device address between 8, 9, 10, or 11 with settings saved persistently in on-board flash.

---

## Emulation Status & Known Limitations

Pico1541 actively strives for cycle-exact 1541 hardware behavior. However, full compatibility remains an ongoing development effort:

- **Cycle Accuracy Status:** While the micro-instruction 6502 core and dual 6522 VIA emulation pass instruction-level test suites and run most fastloaders, cycle-accurate behavior is **not yet 100% complete**. A small number of advanced demos utilizing extreme cycle-tight fastloaders do not yet load correctly.
- **No Weak Bit / Unstable Flux Support:** The magnetic surface engine faithfully reproduces GCR track bitstreams, but currently has **no support for "weak bit" (unstable/fuzzy flux) emulation**. Copy-protected software that verifies weak bits (expecting non-deterministic byte values across consecutive track revolutions) will fail protection checks.
- **Disk Image Formats:** Native emulation supports `.D64`, `.G64`, and `.PRG`. Raw `.NIB` format images are not supported directly in firmware; users should convert `.NIB` captures to `.G64` using external utilities (such as `nibconv`) prior to loading.

---

## Architecture

Pico1541 divides responsibilities cleanly across the RP2350's two Cortex-M33 cores:

```
┌────────────────────────────────────────┐  ┌────────────────────────────────────────┐
│         Core 0 — Drive Manager         │  │        Core 1 — Hardware Emulator      │
│  (Embassy Async / pico1541-manager)    │  │   (Cycle-Exact / pico1541-emulator)    │
├────────────────────────────────────────┤  ├────────────────────────────────────────┤
│ • MicroSD FAT32 Driver (SPI)           │  │ • 6502 CPU Core (racc6502v4) @ 1 MHz   │
│ • Internal Flash LittleFS Storage      │  │ • Dual 6522 VIAs (Timers, Handshake)   │
│ • D64 / G64 / PRG Handlers             │  │ • PIO Magnetic Surface Engine          │
│ • On-the-fly GCR Track Encoder         │  │ • Hardware DMA Ping-Pong Streaming     │
│ • SSD1306 OLED UI & Keypad Task       │  │ • Physical IEC Bus Line Polling        │
│ • SD2IEC CD: Command Interceptor       │  │ • Stepper Motor & Head Position Logic  │
└───────────────────┬────────────────────┘  └───────────────────┬────────────────────┘
                    │                                           │
                    └─────────── Shared Track Buffer ───────────┘
                                (Double-buffered GCR)
```

Communication between cores occurs via atomic flags and shared double-buffered track memory (`DualCoreTrackBuffer`), ensuring Core 1 never suffers jitter or interrupt latency from SD card I/O.

---

## Hardware & Pinout

Pico1541 interfaces with the 5V Commodore IEC bus using an open-collector inverter (7406 or 7405) with 1kΩ pull-up resistors on the 3.3V side. Ready-to-order KiCad PCB schematics and Gerbers are located in [`hardware/kicad/Pico1541v2/`](hardware/kicad/Pico1541v2/).

### Hardware Revision 2 (RP2350 / Raspberry Pi Pico 2)

| GPIO | Function | Description | Connection |
|:---:|:---|:---|:---|
| **GPIO 19** | `DATA_RX` | IEC Data In | Inverted from IEC Pin 5 (DATA, 1kΩ pull-up) |
| **GPIO 20** | `DATA_TX` | IEC Data Out | Drives 7406/7405 Open-Collector to IEC DATA |
| **GPIO 21** | `CLK_RX`  | IEC Clock In | Inverted from IEC Pin 4 (CLK, 1kΩ pull-up) |
| **GPIO 22** | `CLK_TX`  | IEC Clock Out | Drives 7406/7405 Open-Collector to IEC CLK |
| **GPIO 26** | `ATN_RX`  | IEC Attention In | Inverted from IEC Pin 3 (ATN, 1kΩ pull-up) |
| **GPIO 27** | `RESET_RX`| IEC Reset In | Inverted from IEC Pin 6 (RESET, 1kΩ pull-up) |
| **GPIO 4**  | `I2C0_SDA`| OLED Display SDA | SSD1306 I2C Data |
| **GPIO 5**  | `I2C0_SCL`| OLED Display SCL | SSD1306 I2C Clock |
| **GPIO 6..9**| `BUTTONS` | Tactile Buttons | Short to Ground (Up, Down, Enter, Back) |
| **GPIO 11** | `SD_DET`  | MicroSD Card Detect | SD Socket Card Detect Switch |
| **GPIO 12** | `SD_MISO` | MicroSD Data Out | SPI1 MISO (DAT0) |
| **GPIO 13** | `SD_CS`   | MicroSD Chip Select | SPI1 Chip Select (CD) |
| **GPIO 14** | `SD_CLK`  | MicroSD Clock | SPI1 SCK |
| **GPIO 15** | `SD_MOSI` | MicroSD Data In | SPI1 MOSI (CMD) |
| **GPIO 2**  | `PIEZO`   | Piezo Speaker | Head Stepper Sound Simulation |
| **GPIO 10** | `ACT_LED1`| Activity LED Pin 1 | Dual-color Activity Indicator |
| **GPIO 3**  | `ACT_LED2`| Activity LED Pin 2 | Dual-color Activity Indicator |
| **GPIO 16** | `RGB_RED` | Status RGB LED Red | Drive State Indicator |
| **GPIO 17** | `RGB_GREEN`| Status RGB LED Green | Drive State Indicator |
| **GPIO 18** | `RGB_BLUE`| Status RGB LED Blue | Drive State Indicator |

---

## ROM Installation & Legal Notice

> [!IMPORTANT]
> **Commodore DOS ROMs are proprietary and copyrighted.**
> This repository **does NOT contain any Commodore ROM files**.
> To use Pico1541, you must supply a legally obtained 16 KB (16,384 bytes) binary dump of a 1541 DOS ROM (such as `1541-II.251968-03.bin`, original `1541-c.bin`, or a licensed JiffyDOS 1541 ROM).

The ROM file must have an exact size of **16,384 bytes** (`0x4000` bytes).

### Compile-Time Embedding (Flash LittleFS)

This is the standard and recommended installation method. The build system packages the ROMs into an internal LittleFS flash partition on the Pico 2, making the drive completely self-contained.

1. Install `mklittlefs` on your system:
   - On Linux / macOS, build from source via [earlephilhower/mklittlefs](https://github.com/earlephilhower/mklittlefs) or install via your package manager.
   - Verify `mklittlefs` is accessible in your `PATH` (`mklittlefs --version`).
2. Copy your 16 KB ROM file(s) into the `flash-fs/roms/` directory in this repository:
   ```bash
   cp /path/to/1541-II.251968-03.bin flash-fs/roms/
   ```
   *(You can place multiple `.bin` or `.rom` files here—such as standard 1541-II DOS, 1541-C, and JiffyDOS—to switch between them on device).*
3. Compile the firmware as described in [Building & Flashing](#building--flashing). The build script (`build.rs`) will automatically pack `flash-fs/` into a LittleFS image (`fs.bin`) and link it into flash memory.

### Switching ROMs at Runtime (OLED Menu)

When multiple ROMs are bundled into the flash filesystem, you can switch between them on the fly:

1. On the device, navigate to:
   ```
   Main Menu -> Settings -> Select ROM
   ```
2. The OLED screen displays all ROMs discovered in internal storage.
3. Scroll to the desired ROM and press **Select**. The firmware immediately writes the selection to `/settings/boot_rom.txt` in persistent flash, reloads the emulator memory buffer, and resets the 6502 drive emulation.

---

## Prerequisites

### 1. Rust Toolchain

Pico1541 is built with Rust. Install the stable Rust toolchain via `rustup`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. ARM Cortex-M Target

Add the bare-metal target for the RP2350 (Cortex-M33):

```bash
rustup target add thumbv8m.main-none-eabihf
```

### 3. Flashing Tools

Install `elf2uf2-rs` for easy USB drag-and-drop flashing (when the Pico 2 is in BOOTSEL mode):

```bash
cargo install elf2uf2-rs --locked
```

Alternatively, install [`picotool`](https://github.com/raspberrypi/picotool) or [`probe-rs`](https://probe.rs/) if you prefer flashing via SWD/JTAG debuggers.

---

## Building & Flashing

Clone the repository and enter the project directory:

```bash
git clone https://github.com/johnnyegel/Pico1541.git
cd Pico1541
```

Make sure you have placed your 16 KB DOS ROM in `flash-fs/roms/` before building.

### Building the Release Binary

Build the optimized release firmware:

```bash
cargo build -p pico1541-rp2350 --target thumbv8m.main-none-eabihf --release
```

### Flashing via USB (BOOTSEL Mode)

1. Unplug the Pico 2.
2. Hold down the **BOOTSEL** button on the Pico 2 board and plug it into your computer via USB.
3. Flash the binary:
   ```bash
   elf2uf2-rs -d target/thumbv8m.main-none-eabihf/release/pico1541-rp2350
   ```
   *(Or run `cargo run -p pico1541-rp2350 --release` directly from within the `crates/pico1541-rp2350` directory).*

### Flashing via SWD Debugger

If you are using a Raspberry Pi Debug Probe or SWD debugger with `probe-rs`, you can flash using:

```bash
cargo flash-swd
```

---

## Usage & Operation

### IEC Cable Connection

Connect a standard 6-pin DIN serial cable from the Commodore 64 / 128 serial port to the DIN connector on the Pico1541 hat. Ensure the ground reference is shared.

### OLED Menu Navigation

Pico1541 uses a 4-button navigation scheme:

- **Up / Down:** Scroll through menus, files, and disk directories.
- **Select / Enter:** Open a directory, mount a disk image (`.D64`, `.G64`), or mount a `.PRG`.
- **Back:** Navigate up one directory level, or unmount the current image and return to the main menu.
- **Fast Scroll:** Holding the Up or Down buttons skips through large lists quickly.

### C64 Commands & Browser

Once an image is mounted, use standard Commodore commands on your C64:

```basic
LOAD "$",8
LIST
LOAD "*",8,1
RUN
```

#### Navigating with C64 File Browsers (e.g., `fb64`):

Pico1541 includes built-in SD2IEC protocol command snooping. When using tools like `fb64`:
- Directory changes (`CD:DIRNAME`, `CD:_`, `CD:←`) are automatically intercepted on command channel 15.
- The virtual directory track is dynamically updated, allowing you to browse folders and mount disks directly from the C64 screen without touching the hardware buttons.

---

## Repository Structure

```
Pico1541/
├── crates/
│   ├── pico1541-emulator/     # Pure 6502 and dual 6522 VIA emulation engine
│   ├── racc6502v4/            # Cycle-accurate micro-step 6502 CPU implementation
│   ├── pico1541-manager/      # Async Embassy manager, disk formats, GCR, UI logic
│   ├── pico1541-rp-common/    # Shared hardware drivers (PIO, SPI, Display)
│   ├── pico1541-rp2350/       # Firmware entry point for Raspberry Pi Pico 2 (RP2350)
│   └── pico1541-common/       # Common definitions, events, and inter-core buffers
├── flash-fs/                  # Root filesystem packed into on-chip LittleFS partition
│   └── roms/                  # Location to place 16KB DOS ROMs for compile-time embedding
├── hardware/                  # KiCad schematics, PCB layouts, and Gerber files
│   └── kicad/
│       └── Pico1541v2/        # Hardware Revision 2 interface hat (RP2350)
├── COMMERCIAL_LICENSE.md      # Commercial manufacturing terms
└── LICENSE.md                 # Firmware license (PolyForm Noncommercial 1.0.0)
```

---

## Licensing

This project is made available under multiple licenses:

- **Firmware & Management Software:**
  Licensed under the **[PolyForm Noncommercial License 1.0.0](LICENSE.md)**. Free for personal, hobby, educational, and non-commercial use.
- **6502 Emulator Core (`racc6502v4`):**
  Licensed under the **[GNU General Public License v3.0 (GPL-3.0)](crates/racc6502v4/LICENSE.md)**.
- **Hardware Design & KiCad Schematics:**
  Licensed under the **[Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0)](hardware/LICENSE.md)**.
- **Commercial Licensing:**
  Commercial manufacturing and sale of assembled Pico1541 units requires a commercial license for a flat royalty fee of **5 EUR/USD per sold unit**. Please refer to [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md) or contact [johnnyege@gmail.com](mailto:johnnyege@gmail.com) to arrange an agreement before starting production.
