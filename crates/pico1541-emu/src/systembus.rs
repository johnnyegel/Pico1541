use racc6502::bus::{Bus, MemoryMonitor};
use crate::via6522::Via6522;
use crate::DriveHardware;
use crate::drivemechanics::DriveMechanics;
use log;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
pub struct SystemBus<H: DriveHardware> {
    ram: [u8; RAM_SIZE],
    rom: [u8; ROM_SIZE],
    pub via1: Via6522,
    pub via2: Via6522,
    pub hardware: H,
    pub mechanics: DriveMechanics,
    pub so_active: bool,
    
    // Cached state to detect changes
    last_motor_on: bool,
    last_led_on: bool,
}

impl<H: DriveHardware> SystemBus<H> {
    pub fn new(rom: [u8; ROM_SIZE], hardware: H) -> Self {
        let mut via1 = Via6522::new();
        // Track 0 Sensor is tied to Ground, The rest of the port is unused
        via1.in_a = 0x0;

        Self {
            ram: [0; RAM_SIZE],
            rom,
            via1,
            via2: Via6522::new(),
            hardware,
            mechanics: DriveMechanics::new(),
            so_active: false,
            last_motor_on: false,
            last_led_on: false,
        }
    }

    pub fn tick_cycle(&mut self) {
        if self.mechanics.tick(&mut self.hardware, &mut self.via2) {
            self.so_active = true;
        } else {
            self.so_active = false;
        }

        self.via1.in_b = self.hardware.get_iec_pins();
        self.hardware.set_iec_pins(self.via1.get_port_b());
        self.via1.tick_cycle();
        self.via2.tick_cycle();
    }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self) -> bool {
        self.last_motor_on
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self) -> bool {
        self.last_led_on
    }

    fn check_state_changes(&mut self) {
        // --- Motor (VIA2 Port B bit 2) ---
        // Active-LOW on 1541-II (1 = Motor ON)
        let motor_on = (self.via2.orb & 0x04) != 0;
        if motor_on != self.last_motor_on {
            log::info!("Motor changed: {} -> {}", self.last_motor_on, motor_on);
            self.last_motor_on = motor_on;
            self.hardware.notify_motor_changed(motor_on);
        }

        // --- LED (VIA2 Port B bit 3) ---
        // Active-LOW on 1541-II (1 = LED ON)
        let led_on = (self.via2.orb & 0x08) != 0;
        
        if led_on != self.last_led_on {
            log::info!("LED changed: {} -> {}", self.last_led_on, led_on);
            self.last_led_on = led_on;
            self.hardware.notify_led_changed(led_on);
        }
    }
}

impl<H: DriveHardware> Bus for SystemBus<H> {
    fn read(&mut self, addr: u16) -> u8 {
        self.tick_cycle();
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE],
            0x1800..=0x1BFF => self.via1.read(addr),
            0x1C00..=0x1FFF => self.via2.read(addr),
            0x8000..=0xBFFF => self.rom[(addr - 0x8000) as usize],
            0xC000..=0xFFFF => self.rom[(addr - 0xC000) as usize],
            _ => {
                log::warn!("Unmapped Read: Addr=${:04X}", addr);
                0
            }
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.tick_cycle();
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE] = value,
            0x1800..=0x1BFF => self.via1.write(addr, value),
            0x1C00..=0x1FFF => {
                log::trace!("VIA2 Write: Addr=${:04X}, Val=${:02X}", addr, value);
                self.via2.write(addr, value);
                let reg = addr & 0x0F;
                if reg == 0x00 || reg == 0x0C {
                    self.check_state_changes();
                }
            }
            0x8000..=0xFFFF => {}
            _ => {
                log::warn!("Unmapped Write: Addr=${:04X}, Val=${:02X}", addr, value);
            }
        }
    }

    fn irq_pending(&self) -> bool {
        self.via1.irq || self.via2.irq
    }

    fn so_pending(&self) -> bool {
        self.so_active
    }
}

/// Implement MemoryMonitor for SystemBus to allow peeking at memory without affecting timing.
#[cfg(feature = "debug")]
impl<H: DriveHardware> MemoryMonitor for SystemBus<H> {
    
    fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE],
            0x1800..=0x1BFF => self.via1.peek(addr),
            0x1C00..=0x1FFF => self.via2.peek(addr),
            0x8000..=0xBFFF => self.rom[(addr - 0x8000) as usize],
            0xC000..=0xFFFF => self.rom[(addr - 0xC000) as usize],
            _ => 0,
        }
    }
    
    fn poke(&mut self, _addr: u16, _value: u8) {
        // NOT implemented yet!
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activetrack::ActiveTrack;
    struct MockHardware { track: ActiveTrack }
    impl DriveHardware for MockHardware {
        fn get_active_track(&self) -> &ActiveTrack { &self.track }
        fn notify_head_stepped(&mut self, _ht: u8) {}
        fn notify_track_dirty(&mut self) {}
        fn notify_led_changed(&mut self, _en: bool) {}
        fn notify_motor_changed(&mut self, _en: bool) {}
        fn get_iec_pins(&self) -> u8 { 0xFF }
        fn set_iec_pins(&mut self, _p: u8) {}
    }
    #[test]
    fn test_ram_read_write() {
        let mut pico_bus = SystemBus::new([0; ROM_SIZE], MockHardware { track: ActiveTrack::new() });
        pico_bus.write(0x0100, 0xAB);
        assert_eq!(pico_bus.read(0x0100), 0xAB);
        assert_eq!(pico_bus.read(0x0900), 0xAB);
    }
}
