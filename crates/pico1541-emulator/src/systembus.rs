use core::marker::PhantomData;
use racc6502::bus::Bus;
#[cfg(feature = "debug")]
use racc6502::bus::MemoryMonitor;
use crate::via6522::Via6522;
use crate::interface::HostInterface;
use crate::drivemechanics::DriveMechanics;
use log;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
///
/// `H` identifies the concrete `HostInterface` implementation used at the call
/// site.  No instance of `H` is stored here — it is passed into every
/// `read` / `write` call via the `Bus::Ctx` mechanism and forwarded to
/// `tick_cycle`.
pub struct SystemBus<H: HostInterface> {
    ram: [u8; RAM_SIZE],
    rom: [u8; ROM_SIZE],
    pub via1: Via6522,
    pub via2: Via6522,
    pub mechanics: DriveMechanics,
    pub so_active: bool,
    _phantom: PhantomData<H>,
}

impl<H: HostInterface> SystemBus<H> {
    pub fn new(rom: [u8; ROM_SIZE]) -> Self {
        let mut via1 = Via6522::new();
        // Track 0 Sensor is tied to Ground, The rest of the port is unused
        via1.in_a = 0x0;
        // Default device address to 8 (PB5=0, PB6=0). Also all lines released.
        via1.in_b = 0x0;

        Self {
            ram: [0; RAM_SIZE],
            rom,
            via1,
            via2: Via6522::new(),
            mechanics: DriveMechanics::new(),
            so_active: false,
            _phantom: PhantomData,
        }
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self) -> bool { self.mechanics.led_on(&self.via2) }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self) -> bool { self.mechanics.motor_on(&self.via2) }

    pub fn reset(&mut self) {
        self.ram = [0; RAM_SIZE];
        self.via1.reset();
        self.via2.reset();
        self.mechanics.reset();
        self.so_active = false;
    }

    pub fn tick_cycle(&mut self, hardware: &mut H) {
        if self.mechanics.tick(hardware, &mut self.via2) {
            self.so_active = true;
        } else {
            self.so_active = false;
        }

        let host_data = hardware.read_data();
        let host_clk  = hardware.read_clk();
        let host_atn  = hardware.read_atn();

        // VIA1 outputs: PB1=DATA_OUT, PB3=CLK_OUT, PB4=ATNA (1=Asserted/LOW wire)
        let drv_data = (self.via1.orb & self.via1.ddrb & 0x02) != 0;
        let drv_clk  = (self.via1.orb & self.via1.ddrb & 0x08) != 0;
        // Hardware ATN acknowledge: the drive controller chip XORs ATN_IN with ATNA
        // (PB4 output) and asserts DATA when they differ.  This lets the hardware
        // pull DATA without waiting for the CPU — and releases it once the CPU
        // sets ATNA to match ATN, or re-asserts briefly at end-of-ATN (EOI handshake).
        let atna = (self.via1.orb & self.via1.ddrb & 0x10) != 0;
        let hw_atn_ack = host_atn ^ atna;

        // VIA1 inputs: PB0=DATA_IN, PB2=CLK_IN, PB7=ATN_IN (1=Asserted/LOW wire)
        let mut in_b: u8 = 0;
        if host_data { in_b |= 0x01; }
        if host_clk  { in_b |= 0x04; }
        if host_atn  { in_b |= 0x80; }
        self.via1.in_b = in_b;

        // CA1: fires on positive edge of ATN assertion.
        // host_atn=true (Asserted/LOW wire) -> 7406 output HIGH.
        self.via1.set_ca1(host_atn);

        hardware.drive_data(drv_data || hw_atn_ack);
        hardware.drive_clk(drv_clk);

        self.via1.tick_cycle();
        self.via2.tick_cycle();

        hardware.sync();
    }
}

impl<H: HostInterface> Bus for SystemBus<H> {
    type Ctx = H;

    fn read(&mut self, addr: u16, ctx: &mut H) -> u8 {
        self.tick_cycle(ctx);
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

    fn write(&mut self, addr: u16, value: u8, ctx: &mut H) {
        self.tick_cycle(ctx);
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) % RAM_SIZE] = value,
            0x1800..=0x1BFF => self.via1.write(addr, value),
            0x1C00..=0x1FFF => self.via2.write(addr, value),
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
    impl IecBus for MockHardware {
        fn read_data(&self) -> bool { false }
        fn read_clk(&self) -> bool { false }
        fn read_atn(&self) -> bool { false }
        fn drive_data(&mut self, _: bool) {}
        fn drive_clk(&mut self, _: bool) {}
    }
    impl HostInterface for MockHardware {
        fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
        fn send_event(&mut self, _: DriveEvent) {}
    }
    #[test]
    fn test_ram_read_write() {
        let mut bus = SystemBus::<MockHardware>::new([0; ROM_SIZE]);
        let mut hw = MockHardware { track: ActiveTrack::new() };
        bus.write(0x0100, 0xAB, &mut hw);
        assert_eq!(bus.read(0x0100, &mut hw), 0xAB);
        assert_eq!(bus.read(0x0900, &mut hw), 0xAB);
    }
}
