# Project Context: Pico1541 (Rust)
**Goal:** A highly portable, cycle-exact Commodore 1541 floppy drive emulator targeting the Raspberry Pi Pico (RP2040), written in `no_std` Rust.

## 1. Workspace Architecture
The project uses a Cargo workspace to strictly decouple the hardware-agnostic emulation from the RP2040-specific implementation.
* **`pico1541-rp2040`**: The main entry point. Initializes the RP2040, sets up the dual-core environment, configures PIO, and allocates shared memory.
* **`pico1541-emu`**: Pure `#![no_std]` crate. Contains the cycle-exact 6502 CPU and 6522 VIA emulation. Highly synchronous. Completely hardware-agnostic (runnable on host PC for unit testing).
* **`pico1541-manager`**: Embassy-based async crate. Manages FAT32 filesystems (SD via SPI, USB Host), OLED UI (I2C), and physical buttons. Translates logical files (`.D64`) into physical flux data (`.G64`/GCR).

## 2. Hardware & Electrical Constraints
* **IEC Serial Bus (5V):** The C64 IEC bus (ATN, CLK, DATA, RESET, SRQ) is strictly **open-drain**. 
    * *Hardware Rule:* Must use MOSFET-based bidirectional open-drain level shifters (e.g., BSS138). Standard push-pull shifters will fail.
    * *Software Rule (PIO):* Never drive pins HIGH. Toggle the `PINDIRS` register in PIO assembly to either pull LOW (Output) or float (Input).
* **Storage & UI:** MicroSD (SPI), USB Thumbdrive (Native RP2040 USB Host 1.1), I2C OLED (SSD1306), Tactile buttons.

## 3. Emulation Strategy (Core 0)
* **Strict Original ROMs:** Runs unmodified Commodore ROMs (e.g., 1541-II). No ROM patching.
* **Cycle-Exact Ticking:** Uses the `mos6502` crate. The hardware clock (VIAs, Drive Motor) is ticked *inside* the `get_byte` and `set_byte` methods of the `Bus` trait implementation to ensure 100% cycle accuracy with memory accesses, preventing fastloader crashes.
* **Analog Drive Mechanics:** A `DriveMechanics` struct simulates the physical disk rotation over a shared memory buffer. When a GCR byte is fully read, it latches into VIA2 and physically pulses the 6502's `SO` (Set Overflow) pin.

## 4. Manager Strategy (Core 1 - Embassy)
* **Async Concurrency:** Runs the `embassy` framework. Separate async tasks handle the UI, buttons, and storage without blocking each other.
* **On-the-Fly GCR Conversion:** Core 1 reads `.D64` sectors and dynamically constructs the physical track layout (Sync marks $FF, Header blocks, Data blocks, and Gap bytes) encoded in 5-bit GCR, placing it into shared memory.

## 5. Inter-Core Communication (The Hardware Boundary)
The emulator and the manager communicate purely through lock-free structures. 
* **`ActiveTrack` (Shared Memory):** A static struct using `core::sync::atomic` containing a single physical track's GCR bitstream (`[u8; MAX_TRACK_BYTES]`). Core 1 writes to it; Core 0 reads from it.
* **SPSC Command Queue:** Uses `heapless::spsc::Queue`. Core 0 pushes events (e.g., `Command::StepHead`) to tell Core 1 to load a new track from storage.
* **Trait Injection:** `pico1541-emu` relies on a `DriveHardware` trait to access these structures, allowing standard `cargo test` runs on the host PC using mock vectors instead of actual hardware queues.

## 6. Hybrid IEC Bus Handling (PIO)
* **Mode 1: Interceptor:** On boot, PIO acts as a shift register, quietly capturing standard C64 DOS commands (e.g., `OPEN` for directory browsing) without waking the 6502 emulator.
* **Mode 2: Transparent Bridge:** Once a disk is mounted, PIO swaps to a high-speed transparent bridge. It syncs the physical open-drain pins directly to virtual registers mapped in Core 0's memory, allowing the emulated 6522 VIAs to control the bus directly for cycle-exact copy protection compatibility.
