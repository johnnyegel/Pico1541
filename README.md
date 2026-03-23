# Pico1541

Pico1541 is a CBM 1541 cycle accurate disk drive emulator running on the Raspberry Pi Pico.
It emulates the exact hardware of the original drive and is therefore compatible with all
fast loaders and custom roms supported by the original drives.

## Hardware

The Pico1541 runs on Raspberry PI Pico, mounted on a custom interface "hat" containing the 
necessary hardware to interact with the C64 hardware through the IEC bus. The interface can
support an SD card reader, and USB flash drives. It also supports buttons for selecting 
what image to mount in the drive.

## Firmware

The PI Pico firmware is implemented in Rust, and currently runs on the original PI Pico (1).

