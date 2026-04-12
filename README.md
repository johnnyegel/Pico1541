# Pico1541

Pico1541 is a CBM 1541 cycle accurate disk drive emulator running on the Raspberry Pi Pico.
It emulates the exact hardware of the original drive and is therefore compatible with all
fast loaders and custom roms supported by the original drives.

## Hardware

The Pico1541 runs on Raspberry PI Pico, mounted on a custom interface "hat" containing the 
necessary hardware to interact with the C64 hardware through the IEC bus. The interface can
support an SD card reader, and USB flash drives. It also supports buttons for selecting 
what image to mount in the drive.

The hardware design is created with KiCad, and is Licensed under [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/)
See the [Hardware License](hardware/LICENSE.md) file for details.

See the [Commercial Lisence](COMMERCIAL_LICENSE.md) file for commercial lisencing options.

## Firmware

The PI Pico firmware is implemented in Rust, and currently runs on the original PI Pico (1).

The firmware and code is Licensed under [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0/).
See the [Firmware License](LICENSE.md) file for details.

See the [Commercial Lisence](COMMERCIAL_LICENSE.md) file for commercial lisencing options.

## Cycle Accurate 6502 Emulator (racc6502)

This was needed for cycle accurate 1541 drive emulation, and I could not find any existing projects
that was suitable. So I created my own which use micro-instructions to make the emulated CPU behave
exactly as the original CPU would.

The 6502 emulator is Licensed under [GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.en.html).
See the [racc6502 License](racc6502/LICENSE.md) file for details.
