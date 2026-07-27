# Issues after timing refactoring

- Track changes are not working correctly

- Catch up seems bizarre. Why first spin until, and then catch up? I would think we wanted to catch up
first, and then spin until it's time for the next instruction.

See emulator crate in lib:
```
diff --git a/crates/pico1541-emulator/src/lib.rs b/crates/pico1541-emulator/src/lib.rs
index b881a65..363af1b 100644
--- a/crates/pico1541-emulator/src/lib.rs
+++ b/crates/pico1541-emulator/src/lib.rs
@@ -107,8 +107,6 @@ impl<H: HostInterface, S: MagneticSurface> Emulator<H, S> {
     #[link_section = ".data.ram_func"]
     #[inline(always)]
     pub fn tick(&mut self, hw: &mut H) -> u32 {
-        let pc = self.cpu.pc;
-        hw.debug_pin_1(pc >= 0x0300 && pc < 0x0800);

         #[cfg(feature = "flight-recorder")]
         unsafe {
@@ -143,11 +141,13 @@ impl<H: HostInterface, S: MagneticSurface> Emulator<H, S> {
         // Poll IEC and DriveMechanics on every instruction for real-time accuracy without jitter
         self.memory.poll_realtime(hw, elapsed);

-        hw.debug_pin_1(false);
         if self.cpu.cycles.wrapping_sub(self.memory.next_sync_cycle) < 0x80000000 || self.memory.sync_required {
-            self.memory.catch_up(self.cpu.cycles);
-            self.memory.sync_required = false;
+            hw.spin_until(self.cpu.cycles);
         }
+
+        hw.clear_debug_pins();
+        self.memory.catch_up(self.cpu.cycles);
+        self.memory.sync_required = false;

         if self.memory.so_pulsed {
             self.memory.so_pulsed = false;
@@ -172,8 +172,6 @@ impl<H: HostInterface, S: MagneticSurface> Emulator<H, S> {
                 _padding: 0,
             });
         }
-
-        hw.debug_pin_0(false);
         self.cycles = self.cpu.cycles;
         elapsed as u32
     }
```