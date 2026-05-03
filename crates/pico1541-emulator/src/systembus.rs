use crate::drivemechanics::DriveMechanics;
use crate::interface::{HostInterface, IecBusState};
use crate::via6522::Via6522;
use core::marker::PhantomData;
use log;
use racc6502::bus::Bus;
#[cfg(feature = "debug")]
use racc6502::bus::MemoryMonitor;

const RAM_SIZE: usize = 0x0800; // 2KB
const ROM_SIZE: usize = 0x4000; // 16KB

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
///
/// `H` identifies the concrete `HostInterface` implementation used at the call
/// site.  No instance of `H` is stored here — it is passed into every
/// `read` / `write` call via the `Bus::Ctx` mechanism and forwarded to
/// `tick_cycle`.
pub struct SystemBus<H: HostInterface> {
    pub ram: [u8; RAM_SIZE],
    rom: [u8; ROM_SIZE],
    pub via1: Via6522,
    pub via2: Via6522,
    pub mechanics: DriveMechanics,
    pub so_active: bool,
    pub device_id: u8,
    _phantom: PhantomData<H>,
}

impl<H: HostInterface> SystemBus<H> {
    pub fn new(rom: [u8; ROM_SIZE], device_id: u8) -> Self {
        // :: Intialize VIA1
        let mut via1 = Via6522::new();

        let systembus = Self {
            ram: [0; RAM_SIZE],
            rom,
            via1,
            via2: Via6522::new(),
            mechanics: DriveMechanics::new(),
            so_active: false,
            device_id,
            _phantom: PhantomData,
        };

        // Set up the VIA1 initial state
        via1.in_a = 0x0;
        via1.in_b = systembus.host_iec_to_via(&IecBusState::default());

        systembus
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self) -> bool {
        self.mechanics.led_on(&self.via2)
    }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self) -> bool {
        self.mechanics.motor_on(&self.via2)
    }

    pub fn reset(&mut self) {
        self.ram = [0; RAM_SIZE];
        self.via1.reset();
        self.via2.reset();
        self.mechanics.reset();
        self.so_active = false;
    }

    pub fn tick_cycle(&mut self, hardware: &mut H) {
        if hardware.pop_discard_request() {
            self.discard_command_buffer();
        }

        if self.mechanics.tick(hardware, &mut self.via2) {
            self.so_active = true;
        } else {
            self.so_active = false;
        }

        // Reslove the Host IecBusState
        let host_iec = if hardware.is_hijacked() {
            IecBusState::default()
        } else {
            hardware.iec_host()
        };

        // Update VIA1's input register B with host IEC state
        self.via1.in_b = self.host_iec_to_via(&host_iec);
        // CA1: fires on positive edge of ATN assertion.
        // host_atn=true (Asserted/LOW wire) -> 7406 output HIGH.
        self.via1.set_ca1(host_iec.get_atn());

        // Execute the VIA cycles, so we get the actual output given the input state
        self.via1.tick_cycle();
        self.via2.tick_cycle();

        // Resolve the IEC lines from the drive. ATN is set to ATNA (PB4) 
        let drive_iec = if hardware.is_hijacked() {
            IecBusState::default()
        } else {
            self.drive_iec_state(&host_iec)
        };

        // Set the IEC drive lines
        hardware.iec_drive(drive_iec);
    }

    /// Silently discards a command so the 1541 DOS parser ignores it.
    pub fn discard_command_buffer(&mut self) {
        // 1. Set command length to 0.
        // The DOS parser will see an empty string and skip execution.
        self.ram[0x02A7] = 0;

        // 2. Force the internal error state to "00, OK" just to be safe.
        // This ensures the green LED stays solid and no error is returned.
        self.ram[0x026C] = 0;
    }

    #[inline]
    fn drive_iec_state(&self, host_iec: &IecBusState) -> IecBusState {
        // Get the drive ATNA bit, and align it to bit 2 for the IecBusState bitmask.
        let atna = (self.via1.orb & self.via1.ddrb & 0x10) >> 2;
        // XOR with the host ATN to get the hardware ATN acknowledge state, 
        // which is what the drive actually puts on the DATA line.
        let hw_atn_ack = ((host_iec.0 & IecBusState::ATN_BIT) ^ atna) >> 2;

        // Get the clock and data bits from VIA1 and align them to bits 0 and 1 for the IecBusState bitmask.
        let data_bit = (self.via1.orb & self.via1.ddrb & 0x02) >> 1;
        let clk_bit = (self.via1.orb & self.via1.ddrb & 0x08) >> 2;

        // Or the bits together to get the combined drive state for the IecBusState bitmask.
        IecBusState(clk_bit | data_bit | hw_atn_ack)
    }

    #[inline]
    fn host_iec_to_via(&self, host_iec: &IecBusState) -> u8 {
        // Map the host IEC state to the VIA1 input bits for PB0, PB2, and PB7.
        let data = (host_iec.0 & IecBusState::DATA_BIT) << 0; // PB0 = DATA_IN
        let clk = (host_iec.0 & IecBusState::CLK_BIT) << 1; // PB2 = CLK_IN
        let atn = (host_iec.0 & IecBusState::ATN_BIT) << 5; // PB7 = ATN_IN

        // Shift the drive ID bits into PB5/PB6 and combine with the host IEC bits.
        let device_id_bits = (self.device_id & 0x03) << 5;

        // Or all together to get the full PB input value for VIA1.
        device_id_bits | atn | clk | data
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
            0x0000..=0x17FF => {
                self.ram[(addr as usize) % RAM_SIZE] = value;
                if addr == 0x0077 || addr == 0x0078 {
                    ctx.update_iec_addresses(self.ram[0x0077], self.ram[0x0078]);
                }
            }
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
    struct MockHardware {
        track: ActiveTrack,
    }
    
    impl IecBus for MockHardware {
        fn iec_host(&self) -> IecBusState {
            IecBusState::default()
        }
        fn iec_drive(&mut self, _: IecBusState) {}
    }

    impl HostInterface for MockHardware {
        fn get_active_track(&mut self) -> &mut ActiveTrack {
            &mut self.track
        }
        fn send_event(&mut self, _: DriveEvent) {}
    }
    #[test]
    fn test_ram_read_write() {
        let mut bus = SystemBus::<MockHardware>::new([0; ROM_SIZE], 8);
        let mut hw = MockHardware {
            track: ActiveTrack::new(),
        };
        bus.write(0x0100, 0xAB, &mut hw);
        assert_eq!(bus.read(0x0100, &mut hw), 0xAB);
        assert_eq!(bus.read(0x0900, &mut hw), 0xAB);
    }
}
