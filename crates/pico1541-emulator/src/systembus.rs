use crate::drivemechanics::DriveMechanics;
use crate::via6522::Via6522;
use pico1541_common::interface::{HostInterface, IecBusState};
use core::marker::PhantomData;
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
    pub cycle_counter: u32,
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
            cycle_counter: 0,
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

    #[link_section = ".ramfunc"]
    #[inline(always)]
    pub fn tick_cycle(&mut self, hardware: &mut H) {
        // Resolve the Host IecBusState once per cycle
        let host_iec = hardware.iec_host();
        
        // Pass the host state to sync
        hardware.sync(host_iec);

        // Check for hardware configuration changes every 1024 cycles (~1ms)
        self.cycle_counter = self.cycle_counter.wrapping_add(1);
        if (self.cycle_counter & 0x3FF) == 0 {
            self.device_id = hardware.get_device_id();

            if hardware.pop_discard_request() {
                crate::hle::HleManager::intercept_command(self);
            }
        }

        let requested_sector = self.ram[0x000C];
        /*
        let track = hardware.get_active_track();
        if !matches!(track.layout, pico1541_common::formats::TrackLayout::Void) {
            emu_debug!("Tick: PC=${:04X}, Sector={}, Layout={:?}", self.pc(), requested_sector, track.layout);
        }
        */
        self.so_active = self.mechanics.tick(hardware, &mut self.via2, requested_sector);

        // Update VIA1's input register B with host IEC state
        self.via1.in_b = self.host_iec_to_via(&host_iec);
        // CA1: fires on positive edge of ATN assertion.
        // host_atn=true (Asserted/LOW wire) -> 7406 output HIGH.
        self.via1.set_ca1(host_iec.get_atn());

        // Execute the VIA cycles, so we get the actual output given the input state
        self.via1.tick_cycle();
        self.via2.tick_cycle();

        if self.via1.irq || self.via2.irq {
            // emu_debug!("IRQ Pending: VIA1={:02X}, VIA2={:02X}", self.via1.ifr, self.via2.ifr);
        }

        // Resolve the IEC lines from the drive. ATN is set to ATNA (PB4) 
        let mut drive_iec = self.drive_iec_state(&host_iec);

        if hardware.is_hijacked() {
            // "Yank the cable" from the host's perspective: the drive stops pulling any lines.
            drive_iec = IecBusState::default();
        }

        // Set the IEC drive lines
        hardware.iec_drive(drive_iec);
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
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

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn host_iec_to_via(&self, host_iec: &IecBusState) -> u8 {
        // Map the host IEC state to the VIA1 input bits for PB0, PB2, and PB7.
        let data = host_iec.0 & IecBusState::DATA_BIT; // PB0 = DATA_IN
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

    #[link_section = ".ramfunc"]
    fn read(&mut self, addr: u16, ctx: &mut H) -> u8 {
        self.tick_cycle(ctx);
        if addr >= 0x8000 {
            return self.rom[(addr & 0x3FFF) as usize];
        }
        match addr {
            0x0000..=0x17FF => self.ram[(addr as usize) & (RAM_SIZE - 1)],
            0x1800..=0x1BFF => self.via1.read(addr),
            0x1C00..=0x1FFF => self.via2.read(addr),
            _ => {
                emu_warn!("Unmapped Read: Addr=${:04X}", addr);
                0
            }
        }
    }

    #[link_section = ".ramfunc"]
    fn write(&mut self, addr: u16, value: u8, ctx: &mut H) {
        self.tick_cycle(ctx);
        if addr < 0x1800 {
            self.ram[(addr as usize) & (RAM_SIZE - 1)] = value;
            if addr == 0x0077 || addr == 0x0078 {
                ctx.update_iec_addresses(self.ram[0x0077], self.ram[0x0078]);
            }
            return;
        }
        match addr {
            0x1800..=0x1BFF => self.via1.write(addr, value),
            0x1C00..=0x1FFF => self.via2.write(addr, value),
            0x8000..=0xFFFF => {}
            _ => {
                emu_warn!("Unmapped Write: Addr=${:04X}, Val=${:02X}", addr, value);
            }
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn irq_pending(&self) -> bool {
        self.via1.irq || self.via2.irq
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn nmi_pending(&self) -> bool {
        false
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
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
    use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
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
        fn sync(&mut self, _: IecBusState) {}
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
