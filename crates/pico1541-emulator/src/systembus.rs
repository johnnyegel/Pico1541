use crate::drivemechanics::DriveMechanics;
use crate::via6522::Via6522;
#[cfg(feature = "debug")]
use crate::MemoryMonitor;
use pico1541_common::interface::{HostInterface, IecBusState};
use core::marker::PhantomData;

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
    rom: &'static [u8; ROM_SIZE],
    pub via1: Via6522,
    pub via2: Via6522,
    pub mechanics: DriveMechanics,
    pub so_active: bool,
    pub device_id: u8,
    cycle_counter: u32,
    device_id_pb_bits: u8,
    drive_iec_base: u8,
    last_host_iec: IecBusState,
    last_drive_iec: IecBusState,
    pub current_hw: *mut H,
    pub last_cycle: u32,
    pub so_pulsed: bool,
    _phantom: PhantomData<H>,
}

    impl<H: HostInterface> SystemBus<H> {
    pub fn new(rom: &'static [u8; ROM_SIZE], device_id: u8, track_handle: pico1541_common::track_buffer::EmulatorHandle) -> Self {
        // :: Intialize VIA1
        let via1 = Via6522::new();

        let mut systembus = Self {
            ram: [0; RAM_SIZE],
            rom,
            via1,
            via2: Via6522::new(),
            mechanics: DriveMechanics::new(track_handle),
            so_active: false,
            device_id,
            cycle_counter: 0,
            device_id_pb_bits: (device_id & 0x03) << 5,
            drive_iec_base: 0,
            last_host_iec: IecBusState::default(),
            last_drive_iec: IecBusState::default(),
            current_hw: core::ptr::null_mut(),
            last_cycle: 0,
            so_pulsed: false,
            _phantom: PhantomData,
        };

        // Set up the VIA1 initial state
        systembus.via1.in_a = 0x0;
        systembus.via1.in_b = systembus.map_iec_to_via_in_b(&IecBusState::default());

        systembus
    }

    pub fn update_rom(&mut self, rom: &'static [u8; ROM_SIZE]) {
        self.rom = rom;
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

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn update_drive_iec_base(&mut self, hardware: &mut H) {
        let orb_ddr = self.via1.orb & self.via1.ddrb;
        
        // PB1: DATA OUT, PB3: CLK OUT, PB4: ATNA
        let data_bit = (orb_ddr & 0x02) != 0;
        let clk_bit = (orb_ddr & 0x08) != 0;
        let atna_bit = (orb_ddr & 0x10) != 0;

        let mut base = 0u8;
        if data_bit { base |= 0x01; }
        if clk_bit { base |= 0x02; }
        if atna_bit { base |= 0x04; }

        self.drive_iec_base = base;
        self.update_physical_iec(hardware);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn update_physical_iec(&mut self, hardware: &mut H) {
        let host_atn = self.last_host_iec.get_atn();

        let pb_data = (self.drive_iec_base & 0x01) != 0;
        let pb_clk = (self.drive_iec_base & 0x02) != 0;
        let pb_atna = (self.drive_iec_base & 0x04) != 0;

        // Map ONLY host lines to VIA inputs (Signal Isolation Mandate)
        self.via1.in_b = self.map_iec_to_via_in_b(&self.last_host_iec);

        // Calculate what the drive asserts on the bus
        let mut drive_combined_iec = IecBusState::default();
        drive_combined_iec.set_data(pb_data | (pb_atna ^ host_atn));
        drive_combined_iec.set_clk(pb_clk);


        // Set the IEC drive lines only if they changed
        if drive_combined_iec != self.last_drive_iec {
            hardware.iec_drive(drive_combined_iec);
            self.last_drive_iec = drive_combined_iec;
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick_cycle(&mut self, hardware: &mut H) {
        // Resolve the Host IecBusState once per cycle
        let host_iec = hardware.iec_host();
        
        // Update physical bus and VIA inputs if host OR drive changed
        if host_iec != self.last_host_iec {
            self.last_host_iec = host_iec;
            self.via1.set_ca1(host_iec.get_atn());
            self.update_physical_iec(hardware);
            
            // Notify hardware of the host bus state change
            hardware.sync(host_iec);
        }

        hardware.step();

        // Check for hardware configuration changes every 1024 cycles (~1ms)
        self.cycle_counter = self.cycle_counter.wrapping_add(1);
        if (self.cycle_counter & 0x3FF) == 0 {
            self.device_id = hardware.get_device_id();
            self.device_id_pb_bits = (self.device_id & 0x03) << 5;

            if hardware.pop_discard_request() {
                crate::hle::HleManager::intercept_command(self);
                self.update_physical_iec(hardware);
            }
            if self.mechanics.track_handle.pop_force_disk_change() {
                self.ram[0x001C] = 1;
            }
            
            // 2. Read Write Protect sensor (Active LOW, but C64 logic expects inverted if necessary depending on schematic.
            // On a real 1541: Light sensor logic -> LOW if notch present (write enabled).
            // But we simulate standard `is_write_protected` boolean logic. 
            // In typical emulator mapping: WP bit is 0x10. We follow standard VIA mappings.
            let wp = self.mechanics.track_handle.is_write_protected();
            if wp {
                self.via2.in_b &= !0x10; // LOW = Write protected
            } else {
                self.via2.in_b |= 0x10;  // HIGH = Not write protected
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
        let requested_sector = self.ram[0x000C];
        self.so_active = self.mechanics.tick(hardware, &mut self.via1, &mut self.via2, requested_sector);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn map_iec_to_via_in_b(&self, bus_state: &IecBusState) -> u8 {
        // Map the physical IEC state to the VIA1 input bits for PB0, PB2, and PB7.
        // The physical pins are an open-collector bus, so the Host and Drive signals
        // are already combined in hardware. We just map them to the VIA here.
        let data = bus_state.0 & IecBusState::DATA_BIT; // PB0 = DATA_IN
        let clk = (bus_state.0 & IecBusState::CLK_BIT) << 1; // PB2 = CLK_IN
        let atn = (bus_state.0 & IecBusState::ATN_BIT) << 5; // PB7 = ATN_IN

        // Or all together with cached device ID bits to get the full PB input value for VIA1.
        self.device_id_pb_bits | atn | clk | data
    }
}

impl<H: HostInterface> racc6502v3::Bus for SystemBus<H> {
    type Ctx = H;

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn read(&mut self, addr: u16, cycle: u32, ctx: &mut Self::Ctx) -> u8 {
        self.read_with_cycle(addr, cycle, ctx)
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write(&mut self, addr: u16, value: u8, cycle: u32, ctx: &mut Self::Ctx) {
        self.write_with_cycle(addr, value, cycle, ctx)
    }
}

impl<H: HostInterface> SystemBus<H> {
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn catch_up(&mut self, current_cycle: u32, hw: &mut H) {
        let delta = current_cycle.wrapping_sub(self.last_cycle);
        for _ in 0..delta {
            self.tick_cycle(hw);
            if self.so_pending() {
                self.so_pulsed = true;
            }
        }
        self.last_cycle = current_cycle;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn read_with_cycle(&mut self, addr: u16, cycle: u32, hw: &mut H) -> u8 {
        if addr >= 0x8000 {
            return unsafe { *self.rom.get_unchecked((addr & 0x3FFF) as usize) };
        }
        if addr < 0x1800 {
            return unsafe { *self.ram.get_unchecked((addr as usize) & (RAM_SIZE - 1)) };
        }
        self.catch_up(cycle, hw);
        match addr {
            0x1800..=0x1BFF => self.via1.read(addr),
            0x1C00..=0x1FFF => self.via2.read(addr),
            _ => 0,
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn write_with_cycle(&mut self, addr: u16, value: u8, cycle: u32, ctx: &mut H) {
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

        self.catch_up(cycle, ctx);
        match addr {
            0x1800..=0x1BFF => {
                let reg = (addr & 0x0F) as usize;
                self.via1.write(addr, value);
                if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                    self.update_drive_iec_base(ctx);
                }
            }
            0x1C00..=0x1FFF => {
                let reg = (addr & 0x0F) as usize;
                self.via2.write(addr, value);
                if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                    let drive_track = unsafe { *self.ram.get_unchecked(0x0022) };
                    let drive_sector = unsafe { *self.ram.get_unchecked(0x0033) };
                    self.mechanics.update_state(self.via2.orb, self.via2.ddrb, ctx, drive_track, drive_sector);
                }
            }
            _ => {}
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn irq_pending(&self) -> bool {
        self.via1.irq || self.via2.irq
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn nmi_pending(&self) -> bool {
        false
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn so_pending(&self) -> bool {
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
    use pico1541_common::interface::{DriveEvent, HostInterface, IecBus};
    struct MockHardware;

    impl IecBus for MockHardware {
        fn iec_host(&self) -> IecBusState {
            IecBusState::default()
        }
        fn iec_drive(&mut self, _: IecBusState) {}
    }

    impl HostInterface for MockHardware {
        
        fn send_event(&mut self, _: DriveEvent) {}
        fn sync(&mut self, _: IecBusState) {}
    }
    #[test]
    fn test_ram_read_write() {
        static ROM: [u8; ROM_SIZE] = [0; ROM_SIZE];
        let mut bus = SystemBus::<MockHardware>::new(&ROM, 8, Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new())).split().1);
        let mut hw = MockHardware { 
            
        };
        bus.write_with_cycle(0x0100, 0xAB, 0, &mut hw);
        assert_eq!(bus.read_with_cycle(0x0100, 0, &mut hw), 0xAB);
        assert_eq!(bus.read_with_cycle(0x0900, 0, &mut hw), 0xAB);
    }
}
