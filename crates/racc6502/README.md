# Real-Time Cycle Accurate 6502 emulator

This crate implements a real-time cycle accurate 6502 no_std emulator.
Unlike any other 6502 emulator crate for Rust, this one will access the
virtual bus once for every clock cycle of the CPU, like the actual hardware.
This is useful when you need to run timing exact emulation of the CPU. 

For each clock step, the CPU will access the bus exactly once, and either
read or write a byte to the bus.
