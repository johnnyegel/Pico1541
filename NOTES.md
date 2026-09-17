# Fastload issues

- Seems to be slightly slow
- Proably mis-alignment of track position when changing tracks.
- IEC Reset seems to reset head position, but keeps the track buffer
  When reading the track, the disk sees Half-track 22, but the drive emulator is at track 34. Cause the wrong track to be read,
  and the drive (at least fast-loader) fails with an error.

# Fastloaders to test

### 1. Krill's Loader (Kråknet)

  Written by Krill (of Plush/Focus), this is arguably the most famous modern C64 IRQ-loader. It is heavily used by top-tier demogroups because it allows
  massive trackmos to load data and decompress it (usually with Exomizer) perfectly transparently in the background while music and heavy raster effects
  continue uninterrupted. It is incredibly sensitive to serial bus timing and CPU cycle synchronization.

  • Demos to try: "Edge of Disgrace" by Booze Design, "Uncensored" by Booze Design, and "Coma Light 8" by Oxyron.

  ### 2. Spindle (by Linus Åkesson / lft)

  Spindle is an integrated trackmo builder and IRQ loader. It is famous because it automatically calculates the exact memory usage of every effect and
  optimally interleaves sector loading with the demo's execution. It uses a very specialized custom serial protocol to maximize throughput.

  • Demos to try: "Puff", "Craft", and "Kjell" (all by lft).

  ### 3. V-Max! (and variants)

  While originally a commercial game fastloader and copy protection system (used in the late 80s/early 90s), it was heavily adopted and cracked by
  demosceners. It is notorious for using custom GCR formats, reading half-tracks, and executing wildly obfuscated code in the 1541's zero-page.

  • To try: You can test various commercial games that used V-Max! (like Skate or Die or Maniac Mansion) or demos from the early 90s that used ripped V-Max!
  routines.

  ### 4. DreamLoad

  A classic and highly reliable IRQ loader from the 90s and 2000s. It was the backbone of many classic trackmos before Krill's Loader took over the scene.

  • Demos to try: Many releases by Crest, Oxyron, or Smash Designs from the late 90s/early 2000s used variants of this.

  ### 5. Vorpal Loader

  This was Epyx's proprietary commercial loader. While not a "demo" loader, it is a fantastic stress test for a 1541 emulator. It heavily reprograms the VIA
  timers on the drive to achieve its speed and is extremely sensitive to exact timer emulation and interrupt handling.

  • Games to try: California Games, Summer Games II, Winter Games.
  
# TODO

- Problem when chaning from one mount to another. One d64 image is mounted. I step down to disk2.d64 and select it. Nothing indicates a remount in the logs. Unmounting the current image is the same. Probably
the cause of the Edge of discrace demo not working.


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

