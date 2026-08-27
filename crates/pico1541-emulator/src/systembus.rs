use crate::drivemechanics::DriveMechanics;
use crate::via6522::Via6522;
#[cfg(feature = "debug")]
use crate::MemoryMonitor;
use core::marker::PhantomData;
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::HostInterface;
use pico1541_common::magnetic_surface::MagneticSurface;

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
///
/// `H` identifies the concrete `HostInterface` implementation used at the call
/// site.  No instance of `H` is stored here — it is passed into every
/// `read` / `write` call via the `Bus::Ctx` mechanism and forwarded to
/// `tick_cycle`.
pub struct SystemBus<H: HostInterface, S: MagneticSurface> {
    pub memory: crate::memory::Memory,
    pub via1: Via6522,
    pub via2: Via6522,
    pub mechanics: DriveMechanics<S>,
    pub device_id: u8,
    device_id_pb_bits: u8,
    pub time: crate::time::EmulatorTime,
    so_pulsed: bool,
    last_via2_out: u8,
    _phantom: PhantomData<H>,
}

impl<H: HostInterface, S: MagneticSurface> SystemBus<H, S> {
    pub fn new(
        rom: &'static [u8; 16384],
        memory: &'static mut [u8; 32768],
        device_id: u8,
        mechanics: DriveMechanics<S>,
    ) -> Self {
        // :: Intialize VIA1
        let via1 = Via6522::new();

        let mut systembus = Self {
            memory: crate::memory::Memory::new(memory),
            via1,
            via2: Via6522::new(),
            mechanics,
            device_id,
            device_id_pb_bits: (device_id & 0x03) << 5,
            time: crate::time::EmulatorTime::new(),
            so_pulsed: false,
            last_via2_out: 0xFF,
            _phantom: PhantomData,
        };
        
        // :: Load the ROM image into memory
        systembus.update_rom(rom);

        // Set up the VIA1 initial state
        systembus.via1.set_pa_in(0x0);
        systembus.via1.set_pb_in(0x0);

        systembus
    }

    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.memory.load_rom(rom);
    }

    #[cfg(feature = "debug")]
    pub fn led_on(&self) -> bool {
        self.mechanics.led_on(&self.via2)
    }

    #[cfg(feature = "debug")]
    pub fn motor_on(&self) -> bool {
        self.mechanics.motor_on(&self.via2)
    }

    pub fn reset(&mut self, hardware: &mut H) {
        self.memory.reset();
        self.via1.reset();
        self.via2.reset();
        self.mechanics.reset();
        self.device_id_pb_bits = (self.device_id & 0x03) << 5;
        self.via1.set_pb_in(0xFF);
        self.last_via2_out = 0xFF;
        self.mechanics.update_state(self.via2.get_pb_out(), self.last_via2_out, hardware, 0);

        // Force the track manager to load the reset track
        hardware.track_changed(self.mechanics.half_track());
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn sync_iec_bus(&mut self, hardware: &mut H, _current_cycle: u32) {
        // :: Handle IEC via inputs and VIA1 CA1 edge detection
        let iec_in = hardware.read_iec_port() | self.device_id_pb_bits;
        let curr_atn = iec_in & H::VIA1_IEC_MASK_ATN_RX != 0;
        self.via1.set_pb_in(iec_in);
        self.via1.set_ca1(curr_atn);

        // :: Handle VIA1 outputs to IEC bus
        let via1_pb_out = self.via1.get_pb_out();
        hardware.write_iec_port(via1_pb_out);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn poll_io(&mut self, hardware: &mut H, elapsed: u32, current_cycle: u32) {
        self.poll_mechanics(elapsed, current_cycle);
        self.sync_iec_bus(hardware, current_cycle);

        if (current_cycle ^ current_cycle.wrapping_sub(elapsed)) & !0x3FF != 0 {
            self.device_id = hardware.get_device_id();
            self.device_id_pb_bits = (self.device_id & 0x03) << 5;

            let wp = self.mechanics.is_write_protected();
            if wp {
                self.via2.set_pb_in(self.via2.pb_in() & !0x10);
            } else {
                self.via2.set_pb_in(self.via2.pb_in() | 0x10);
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn poll_mechanics(&mut self, elapsed: u32, current_cycle: u32) {

        // Hardware Wiring: Pull bytes from the DMA buffer and wire SYNC/SO directly to VIA2/CPU.
        if self.is_motor_on() {
            let pcr_cb2 = self.via2.pcr() & 0b1110_0000;
            let is_read_mode = pcr_cb2 != 0b1100_0000;

            if is_read_mode {
                let signals = self.mechanics.advance_read_head(elapsed, current_cycle);
                
                let mut new_byte_arrived = false;
                if let Some(byte) = signals.byte_opt {
                    self.via2.set_pa_in(byte);
                    new_byte_arrived = true;
                    // We must assume the byte arrived at the BEGINNING of this elapsed block, 
                    // otherwise subsequent reads may happen too fast before we advance time enough to clear the flag.
                    // If we just use current_cycle, we might be up to 7 cycles too late.
                    self.time.last_byte_cycle = current_cycle.wrapping_sub(elapsed);
                }
                
                // Assert SYNC (Pull PB7 Low). Branchless logic for optimal speed.
                let sync_mask = if signals.is_sync { 0x00 } else { 0x80 };
                self.via2.set_pb_in((self.via2.pb_in() & 0x7F) | sync_mask);
                
                if !signals.is_sync && new_byte_arrived {
                    let pcr_ca2 = self.via2.pcr() & 0b0000_1110;
                    let is_so_enabled = (pcr_ca2 & 0b0000_1010) == 0b0000_1010;
                    if is_so_enabled {
                        self.via2.ca1_irq();
                        self.so_pulsed = true;
                    }
                }
            } else {
                // WRITE mode
                self.mechanics.advance_write_head(elapsed, current_cycle, self.via2.ora());
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn tick_cycles(&mut self, elapsed: u32) {
        
        if self.via1.any_active() {
            self.via1.tick_cycles(elapsed);
        }
        if self.via2.any_active() {
            self.via2.tick_cycles(elapsed);
        }
    }

    #[link_section = ".data.ram_func"]
    #[cold]
    fn read_io(&mut self, addr: u16, cycle: u32, ctx: &mut H) -> u8 {
        self.catch_up(cycle);

        let result = if (addr & 0x0400) == 0 {
            let reg = (addr & 0x0F) as usize;
            if reg == 0x00 || reg == 0x0F {
                // For reads, waiting 0.5 cycles aligns precisely with the Phase 2 falling edge
                // where the 6522 actually latches the input pins.
                ctx.spin_until_half(cycle);
                self.sync_iec_bus(ctx, cycle);
            }
            
            self.via1.read(addr)
        } else {
            self.via2.read(addr)
        };

        result
    }

    #[link_section = ".data.ram_func"]
    #[cold]
    fn write_io(&mut self, addr: u16, val: u8, cycle: u32, ctx: &mut H) {
        self.catch_up(cycle);

        if (addr & 0x0400) == 0 {
            let reg = (addr & 0x0F) as usize;
            self.via1.write(addr, val);
            if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                // Emulate the 6522 VIA's Phase 2 falling-edge latch.
                // The physical output pins transition exactly in the middle of the cycle.
                ctx.spin_until_half(cycle);
                self.sync_iec_bus(ctx, cycle);
            }
        } else {
            let reg = (addr & 0x0F) as usize;
            self.via2.write(addr, val);
            if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                let effective_orb = self.via2.get_pb_out();

                if effective_orb != self.last_via2_out {
                    ctx.write_mechanics_port(effective_orb);
                    self.mechanics.update_state(effective_orb, self.last_via2_out, ctx, cycle);
                    self.last_via2_out = effective_orb;
                }
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn has_irq(&self) -> bool {
        self.via1.has_irq() || self.via2.has_irq()
    }
}

impl<H: HostInterface, S: MagneticSurface> racc6502v4::bus::Bus for SystemBus<H, S> {
    type Ctx = H;

    #[inline(always)]
    #[link_section = ".data.ram_func"]
    fn get_instruction_ptr(&self, addr: u16) -> *const u8 {
        self.memory.get_ptr(addr)
    }

    #[inline(always)]
    #[link_section = ".data.ram_func"]
    fn read(&mut self, hw: &mut H, addr: u16, cycle: u32) -> u8 {
        if (addr & 0x9800) == 0x1800 {
            self.read_io(addr, cycle, hw)
        } else {
            self.memory.read(addr)
        }
    }

    #[inline(always)]
    #[link_section = ".data.ram_func"]
    fn write(&mut self, ctx: &mut Self::Ctx, addr: u16, val: u8, cycle: u32) {
        if (addr & 0x9800) == 0x1800 {
            self.write_io(addr, val, cycle, ctx);
        } else if self.memory.write(addr, val) {
            let (addr77, addr78) = self.memory.get_iec_addresses();
            ctx.update_iec_addresses(addr77, addr78);
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn irq_pending(&self) -> bool {
        self.has_irq()
    }
}

impl<H: HostInterface, S: MagneticSurface> SystemBus<H, S> {
    #[link_section = ".data.ram_func"]
    #[inline]
    pub fn catch_up(&mut self, target_cycle: u32) {
        let elapsed = self.time.consume_catchup_delta(target_cycle);
        if elapsed > 0 {
            self.tick_cycles(elapsed);
        }
    }

    
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn so_pending(&mut self) -> bool {
        let result = self.so_pulsed;
        self.so_pulsed = false;
        result
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn is_motor_on(&self) -> bool {
        (self.via2.get_pb_out() & 0x04) != 0
    }
}

/// Implement MemoryMonitor for SystemBus to allow peeking at memory without affecting timing.
#[cfg(feature = "debug")]
impl<H: HostInterface, S: MagneticSurface> MemoryMonitor for SystemBus<H, S> {
    fn peek<H2: ViaHardware>(
        &mut self,
        addr: u16,
        hw: &mut H2,
    ) -> u8 {
        if (addr & 0x9800) == 0x1800 {
            if addr < 0x1C00 {
                self.via1.peek(addr, hw)
            } else {
                self.via2.peek(addr, hw)
            }
        } else {
            self.memory.read(addr)
        }
    }

    fn poke<H2: ViaHardware>(
        &mut self,
        _addr: u16,
        _value: u8,
        _hw: &mut H2,
    ) {
        // NOT implemented yet!
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pico1541_common::interface::{DriveEvent, HostInterface};
    struct MockHardware;

    impl ViaHardware for MockHardware {}

    impl HostInterface for MockHardware {
        fn send_event(&mut self, _: DriveEvent) {}
    }
    #[test]
    fn test_ram_read_write() {
        static ROM: [u8; 16384] = [0; 16384];
        let handle = Box::leak(Box::new(
            pico1541_common::track_buffer::DualCoreTrackBuffer::new(),
        ))
        .split()
        .1;
        let surface = pico1541_common::magnetic_surface::SoftwareMagneticSurface::new();
        let mechanics = crate::drivemechanics::DriveMechanics::new(surface, handle);
        let mut bus = SystemBus::<MockHardware, _>::new(&ROM, alloc::boxed::Box::leak(alloc::boxed::Box::new([0u8; 32768])), 8, mechanics);
        let mut hw = MockHardware {};
        use racc6502v4::bus::Bus;
        bus.write(&mut hw, 0x0100, 0xAB, 0);
        assert_eq!(bus.read(&mut hw, 0x0100, 0), 0xAB);
    }
}
