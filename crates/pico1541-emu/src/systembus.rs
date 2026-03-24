use mos6502::memory::Bus;
use crate::via6522::Via6522;
use crate::DriveHardware;
use crate::drivemechanics::DriveMechanics;
use log::{trace, info, warn};

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
    pub so_pending: bool,
    
    last_motor_on: bool,
    last_led_on: bool,
    pub ticks_this_step: u32,
}

impl<H: DriveHardware> SystemBus<H> {
    pub fn new(rom: [u8; ROM_SIZE], hardware: H) -> Self {
        Self {
            ram: [0; RAM_SIZE],
            rom,
            via1: Via6522::new(),
            via2: Via6522::new(),
            hardware,
            mechanics: DriveMechanics::new(),
            so_pending: false,
            last_motor_on: false,
            last_led_on: false,
            ticks_this_step: 0,
        }
    }

    pub fn tick_cycle(&mut self) {
        self.ticks_this_step += 1;
        if self.mechanics.tick(&mut self.hardware, &mut self.via2) {
            self.so_pending = true;
        }
        self.via1.in_b = self.hardware.get_iec_pins();
        self.hardware.set_iec_pins(self.via1.get_port_b());
        self.via1.tick_cycle();
        self.via2.tick_cycle();
    }

    fn check_state_changes(&mut self) {
        let motor_on = (self.via2.orb & 0x04) == 0; // standard 1541 motor is active-low
        if motor_on != self.last_motor_on {
            info!("Motor changed: {} -> {}", self.last_motor_on, motor_on);
            self.last_motor_on = motor_on;
            self.hardware.notify_motor_changed(motor_on);
        }

        let pcr_ca2 = (self.via2.pcr >> 1) & 0x07;
        let led_on = match pcr_ca2 {
            0b110 => true,
            0b111 => false,
            _ => false,
        };
        if led_on != self.last_led_on {
            info!("LED changed: {} -> {}", self.last_led_on, led_on);
            self.last_led_on = led_on;
            self.hardware.notify_led_changed(led_on);
        }
    }
}

impl<H: DriveHardware> Bus for SystemBus<H> {
    fn get_byte(&mut self, addr: u16) -> u8 {
        self.tick_cycle();
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE],
            0x1800..=0x1BFF => {
                let val = self.via1.read(addr);
                trace!("VIA1 Read: Addr=${:04X}, Val=${:02X}", addr, val);
                val
            }
            0x1C00..=0x1FFF => {
                let val = self.via2.read(addr);
                trace!("VIA2 Read: Addr=${:04X}, Val=${:02X}", addr, val);
                val
            }
            0x8000..=0xBFFF => self.rom[(addr - 0x8000) as usize],
            0xC000..=0xFFFF => self.rom[(addr - 0xC000) as usize],
            _ => {
                warn!("Unmapped Read: Addr=${:04X}", addr);
                0
            }
        }
    }

    fn set_byte(&mut self, addr: u16, value: u8) {
        self.tick_cycle();
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE] = value,
            0x1800..=0x1BFF => {
                trace!("VIA1 Write: Addr=${:04X}, Val=${:02X}", addr, value);
                self.via1.write(addr, value);
            }
            0x1C00..=0x1FFF => {
                trace!("VIA2 Write: Addr=${:04X}, Val=${:02X}", addr, value);
                self.via2.write(addr, value);
                let reg = addr & 0x0F;
                if reg == 0x00 || reg == 0x0C {
                    self.check_state_changes();
                }
            }
            0x8000..=0xFFFF => {}
            _ => {
                warn!("Unmapped Write: Addr=${:04X}, Val=${:02X}", addr, value);
            }
        }
    }

    fn irq_pending(&mut self) -> bool {
        self.via1.irq || self.via2.irq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mos6502::{cpu::CPU, instruction::Nmos6502};
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
        pico_bus.set_byte(0x0100, 0xAB);
        assert_eq!(pico_bus.get_byte(0x0100), 0xAB);
        assert_eq!(pico_bus.get_byte(0x0900), 0xAB);
    }
}
