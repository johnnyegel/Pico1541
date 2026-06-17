with open("crates/pico1541-emulator/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace(
    "pub fn reset(&mut self, hw: &mut H) {\n        emu_debug!(\"Emulator Reset\");\n        self.memory.reset();\n        self.cpu.reset(&mut self.memory, hw);\n    }",
    "pub fn reset(&mut self, hw: &mut H) {\n        emu_debug!(\"Emulator Reset\");\n        self.memory.reset();\n        self.cpu.reset(&mut self.memory, hw);\n        let drive_track = self.memory.ram[0x0022];\n        let drive_sector = self.memory.ram[0x0033];\n        self.memory.mechanics.update_state(self.memory.via2.orb, self.memory.via2.ddrb, hw, drive_track, drive_sector);\n    }"
)

with open("crates/pico1541-emulator/src/lib.rs", "w") as f:
    f.write(content)
