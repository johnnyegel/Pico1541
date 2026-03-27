use racc6502::bus::Bus;
#[cfg(feature = "debug")]
use racc6502::bus::MemoryMonitor;
use crate::via6522::Via6522;
use crate::interface::{DriveEvent, HostInterface, IecBus};
use crate::drivemechanics::DriveMechanics;
use log;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
pub struct SystemBus<H: HostInterface> {
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

impl<H: HostInterface> SystemBus<H> {
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

        // --- IEC Bus open-drain model ---
        // host_bus: 1 = HIGH (released), 0 = LOW (asserted)
        let host = self.hardware.read_iec();

        // VIA1 Drive outputs (open-drain transistors: 1 in ORB = pulls line LOW)
        // PB0 = DATA_OUT, PB2 = CLK_OUT
        let drv_data_lo = (self.via1.orb & 0x01) != 0;
        let drv_clk_lo  = (self.via1.orb & 0x04) != 0;

        // Combined bus: HIGH only if NOBODY pulls it LOW
        let bus_data = host.data() && !drv_data_lo;
        let bus_clk  = host.clk()  && !drv_clk_lo;
        let bus_atn  = host.atn(); // ATN driven only by host (C64)

        // VIA1 in_b: inputs are active-HIGH (1 = line is asserted/LOW)
        // PB7 = DATA_IN, PB6 = CLK_IN, PB3 = ATN_IN
        let mut in_b: u8 = 0;
        if !bus_data { in_b |= 0x80; } // PB7 HIGH when DATA LOW
        if !bus_clk  { in_b |= 0x40; } // PB6 HIGH when CLK LOW
        if !bus_atn  { in_b |= 0x08; } // PB3 HIGH when ATN LOW
        self.via1.in_b = in_b;

        // CA1 = ATN line (raw bus voltage); falling edge = ATN asserted = IRQ
        self.via1.set_ca1(bus_atn);

        // Write combined state back so host can observe the open-drain result
        let mut combined = IecBus::default();
        combined.set_data(bus_data);
        combined.set_clk(bus_clk);
        combined.set_atn(bus_atn);
        self.hardware.write_iec(combined);

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
        // Active-LOW on 1541-II (1 = Motor ON)
        let motor_on = (self.via2.orb & 0x04) != 0;
        if motor_on != self.last_motor_on {
            log::info!("Motor changed: {} -> {}", self.last_motor_on, motor_on);
            self.last_motor_on = motor_on;
            self.hardware.send_event(DriveEvent::MotorChanged(motor_on));
        }

        // --- LED (VIA2 Port B bit 3) ---
        // Active-LOW on 1541-II (1 = LED ON)
        let led_on = (self.via2.orb & 0x08) != 0;
        if led_on != self.last_led_on {
            log::info!("LED changed: {} -> {}", self.last_led_on, led_on);
            self.last_led_on = led_on;
            self.hardware.send_event(DriveEvent::LedChanged(led_on));
        }
    }
}

impl<H: HostInterface> Bus for SystemBus<H> {
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
impl<H: HostInterface> MemoryMonitor for SystemBus<H> {
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
    use crate::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
    struct MockHardware { track: ActiveTrack }
    impl HostInterface for MockHardware {
        fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
        fn send_event(&mut self, _: DriveEvent) {}
        fn read_iec(&self) -> IecBus { IecBus(0xFF) }
        fn write_iec(&mut self, _: IecBus) {}
    }
    #[test]
    fn test_ram_read_write() {
        let mut pico_bus = SystemBus::new([0; ROM_SIZE], MockHardware { track: ActiveTrack::new() });
        pico_bus.write(0x0100, 0xAB);
        assert_eq!(pico_bus.read(0x0100), 0xAB);
        assert_eq!(pico_bus.read(0x0900), 0xAB);
    }
}
