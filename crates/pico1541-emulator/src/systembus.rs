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
    device_id_pb_bits: u8,
    drive_iec_base: u8,
    last_host_iec: IecBusState,
    last_drive_iec: IecBusState,
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
            device_id_pb_bits: (device_id & 0x03) << 5,
            drive_iec_base: 0,
            last_host_iec: IecBusState::default(),
            last_drive_iec: IecBusState::default(),
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
        self.device_id_pb_bits = (self.device_id & 0x03) << 5;
        self.drive_iec_base = 0;
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn update_drive_iec_base(&mut self, hardware: &mut H) {
        let orb_ddr = self.via1.orb & self.via1.ddrb;
        let atna = (orb_ddr & 0x10) >> 2;
        let data_bit = (orb_ddr & 0x02) >> 1;
        let clk_bit = (orb_ddr & 0x08) >> 2;
        self.drive_iec_base = clk_bit | data_bit | atna;
        self.update_physical_iec(hardware);
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn update_physical_iec(&mut self, hardware: &mut H) {
        // Resolve the IEC lines from the drive.
        let host_atn_bit = (self.last_host_iec.0 & IecBusState::ATN_BIT) >> 2;
        let mut drive_iec = IecBusState(self.drive_iec_base ^ host_atn_bit);

        if hardware.is_hijacked() {
            drive_iec = IecBusState::default();
        }

        // Set the IEC drive lines only if they changed
        if drive_iec != self.last_drive_iec {
            hardware.iec_drive(drive_iec);
            self.last_drive_iec = drive_iec;
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    pub fn tick_cycle(&mut self, hardware: &mut H) {
        // Resolve the Host IecBusState once per cycle
        let host_iec = hardware.iec_host();
        
        // Update VIA1's input register B and CA1 if the host state changed
        if host_iec != self.last_host_iec {
            self.via1.in_b = self.host_iec_to_via(&host_iec);
            self.via1.set_ca1(host_iec.get_atn());
            self.last_host_iec = host_iec;
            // ATN change affects physical IEC output (XOR logic)
            self.update_physical_iec(hardware);

            // Notify hardware of the bus state change (e.g. for snooping)
            hardware.sync(host_iec);
        }

        // Check for hardware configuration changes every 1024 cycles (~1ms)
        self.cycle_counter = self.cycle_counter.wrapping_add(1);
        if (self.cycle_counter & 0x3FF) == 0 {
            self.device_id = hardware.get_device_id();
            self.device_id_pb_bits = (self.device_id & 0x03) << 5;

            if hardware.pop_discard_request() {
                crate::hle::HleManager::intercept_command(self);
                self.update_physical_iec(hardware);
            }
        }

        // Execute the VIA cycles only if active
        if self.via1.any_active() {
            self.via1.tick_cycle();
        }
        if self.via2.any_active() {
            self.via2.tick_cycle();
        }

        // Progress the drive mechanics and update SO pin status
        self.so_active = self.mechanics.tick(hardware, &mut self.via2, 0);
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn host_iec_to_via(&self, host_iec: &IecBusState) -> u8 {
        // Map the host IEC state to the VIA1 input bits for PB0, PB2, and PB7.
        let data = host_iec.0 & IecBusState::DATA_BIT; // PB0 = DATA_IN
        let clk = (host_iec.0 & IecBusState::CLK_BIT) << 1; // PB2 = CLK_IN
        let atn = (host_iec.0 & IecBusState::ATN_BIT) << 5; // PB7 = ATN_IN

        // Or all together with cached device ID bits to get the full PB input value for VIA1.
        self.device_id_pb_bits | atn | clk | data
    }
}

impl<H: HostInterface> Bus for SystemBus<H> {
    type Ctx = H;

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn read(&mut self, addr: u16, _ctx: &mut H) -> u8 {
        if addr >= 0x8000 {
            return unsafe { *self.rom.get_unchecked((addr & 0x3FFF) as usize) };
        }
        if addr < 0x1800 {
            return unsafe { *self.ram.get_unchecked((addr as usize) & (RAM_SIZE - 1)) };
        }
        match addr {
            0x1800..=0x1BFF => self.via1.read(addr),
            0x1C00..=0x1FFF => self.via2.read(addr),
            _ => 0,
        }
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn write(&mut self, addr: u16, value: u8, ctx: &mut H) {
        if addr < 0x1800 {
            unsafe { *self.ram.get_unchecked_mut((addr as usize) & (RAM_SIZE - 1)) = value };
            if addr == 0x0077 || addr == 0x0078 {
                ctx.update_iec_addresses(self.ram[0x0077], self.ram[0x0078]);
            }
            return;
        }
        if addr >= 0x8000 {
            return;
        }
        match addr {
            0x1800..=0x1BFF => {
                self.via1.write(addr, value);
                let reg = (addr & 0x0F) as usize;
                if reg == 0x00 || reg == 0x02 {
                    self.update_drive_iec_base(ctx);
                }
            }
            0x1C00..=0x1FFF => {
                self.via2.write(addr, value);
                let reg = (addr & 0x0F) as usize;
                if reg == 0x00 || reg == 0x02 {
                    self.mechanics.update_state(self.via2.orb, self.via2.ddrb, ctx);
                }
            }
            _ => {}
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
