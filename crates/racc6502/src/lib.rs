//! # racc6502
//! 
//! A cycle-accurate, `#![no_std]` MOS 6502 CPU emulator core.
//! 
//! This crate provides a high-fidelity emulation of the NMOS 6502 processor,
//! designed specifically for systems where exact timing and synchronization with
//! external hardware are critical.
//! 
//! ## Key Concepts
//! 
//! ### Cycle-Accuracy via Micro-Ops
//! Unlike traditional emulators, `racc6502` does not execute instructions in monolithic
//! blocks. Instead, it decomposes every instruction into a sequence of **Micro-Operations (uOps)**.
//! Each call to [`CPU::tick`](cpu/struct.CPU.html#method.tick) executes exactly one uOp and
//! performs exactly one bus access (either a read or a write).
//! 
//! ### Bus-Driven Timing
//! The [`Bus`](bus/trait.Bus.html) trait is the heart of the system's timing. Because every 
//! CPU cycle results in a bus access, the `Bus` implementation effectively acts as the 
//! system clock. This allows for perfect interleaving of CPU cycles with other 
//! hardware components (like VIAs or custom logic).
//! 
//! ## Usage
//! 
//! Implement the `Bus` trait to define your memory map and then pass it to a new `CPU` instance.
//! 
//! ```rust
//! use racc6502::bus::Bus;
//! use racc6502::cpu::CPU;
//! 
//! struct MyBus { memory: [u8; 65536] }
//! impl Bus for MyBus {
//!     fn read(&mut self, addr: u16) -> u8 { self.memory[addr as usize] }
//!     fn write(&mut self, addr: u16, val: u8) { self.memory[addr as usize] = val; }
//! }
//! 
//! let mut cpu = CPU::new(MyBus { memory: [0; 65536] });
//! cpu.tick(); // Execute exactly one clock cycle
//! ```

#![cfg_attr(not(test), no_std)]

pub mod bus;
pub mod cpu;
pub mod instructions;

#[cfg(feature = "debug")]
pub mod disasm;
