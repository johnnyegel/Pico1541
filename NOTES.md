# Fastload issues

- Seems to be slightly slow
- Proably mis-alignment of track position when changing tracks.
- IEC Reset seems to reset head position, but keeps the track buffer
  When reading the track, the disk sees Half-track 22, but the drive emulator is at track 34. Cause the wrong track to be read,
  and the drive (at least fast-loader) fails with an error.

# TODO

- The littlefs handling seems to be duplicated in the 2040 and 2350 crates.
- 

- Refactor code to create a crate dedicated for all Core 1 (emulator code), to make SRAM mapping easy and consistent.
 
    emulation::run_emulator()
     └── loop: for _ in 0..500
          └── emulator.tick(hw)
               ├── hw.get_sys_time() (Profiler start)
               ├── self.cpu.step(&mut self.memory, hw)
               │    ├── bus.read(pc)  --> opcode fetch
               │    ├── bus.read(...) --> operand fetches
               │    │    └── SystemBus::read
               │    │         └── (If addr == $18xx) -> read_io()
               │    │             ├── self.catch_up(cycle) -> Ticks timers/mechanics up to now
               │    │             ├── ctx.spin_until(cycle)  <-- WAITS FOR REAL TIME!
               │    │             ├── self.sync_iec_bus(ctx) <-- Samples physical GPIOs
               │    │             └── via1.read()
               │    └── bus.write(...)
               │         └── (If addr == $18xx) -> write_io()
               │             ├── self.catch_up(cycle)
               │             ├── via1.write(val)
               │             ├── ctx.spin_until(cycle)  <-- WAITS FOR REAL TIME!
               │             └── self.sync_iec_bus(ctx) <-- Samples physical GPIOs
               ├── hw.get_sys_time() (Profiler split)
               ├── self.memory.poll_realtime(hw)
               │    ├── (If no IO happened this instruction) -> sync_iec_bus(hw)
               │    └── hardware.step()
               ├── hw.clear_debug_pins()
               ├── hw.get_sys_time() (Profiler split)
               ├── self.memory.catch_up(self.cpu.cycles)
               └── hw.get_sys_time() (Profiler end)

