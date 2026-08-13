use crate::drivemechanics::DriveMechanics;
use crate::via6522::Via6522;
#[cfg(feature = "debug")]
use crate::MemoryMonitor;
use core::marker::PhantomData;
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::HostInterface;
use pico1541_common::magnetic_surface::MagneticSurface;
use pico1541_common::util::bit_align_u8;

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
    last_iec_out: u8,
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
            last_iec_out: 0xFF, // Force first write
            last_via2_out: 0xFF,
            _phantom: PhantomData,
        };
        
        // :: Load the ROM image into memory
        systembus.update_rom(rom);

        // Set up the VIA1 initial state
        systembus.via1.in_a = 0x0;
        systembus.via1.in_b = 0x0;

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
        self.last_iec_out = 0xFF; // Force first write
        self.last_via2_out = 0xFF;
        self.mechanics.update_state(self.via2.get_pb_out(), hardware, 0);

        // Force the track manager to load the reset track
        hardware.track_changed(self.mechanics.half_track());
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn sync_iec_bus(&mut self, hardware: &mut H, current_cycle: u32) {
        // :: Capture the IEC bus state, and update the VIA1 input register
        let iec_in = hardware.read_iec_port() | self.device_id_pb_bits;
        let prev_atn = self.via1.in_b & H::VIA1_IEC_MASK_ATN_RX;
        let curr_atn = iec_in & H::VIA1_IEC_MASK_ATN_RX;
        self.via1.in_b = iec_in;

        // :? Did the ATN state change?
        if prev_atn != curr_atn {
            // -> Yes, then call the VIA1 CA1 edge handler to trigger an interrupt if needed
            self.via1.ca1_edge(curr_atn != 0);
        }

        // :: Calculate the effective VIA1 Port B output
        let mut effective_orb = self.via1.get_pb_out();

        // Auto-acknowledge ATN: if ATN is asseted and ATNA is not asserted
        effective_orb |= bit_align_u8(curr_atn, H::VIA1_IEC_BIT_ATN_RX, H::VIA1_IEC_BIT_DATA_TX)
                & bit_align_u8(!effective_orb, H::VIA1_IEC_BIT_ATN_ACK, H::VIA1_IEC_BIT_DATA_TX)
                & H::VIA1_IEC_MASK_DATA_TX;

        // :: Write the effective VIA1 Port B output to the physical IEC bus
        if effective_orb != self.last_iec_out {
            hardware.write_iec_port(effective_orb);
            self.last_iec_out = effective_orb;
        }
        
        self.time.mark_iec_synced(current_cycle);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn poll_realtime(&mut self, hardware: &mut H, elapsed: u32, current_cycle: u32) {
        if self.time.should_sync_iec(current_cycle) {
            self.sync_iec_bus(hardware, current_cycle);
        }

        if (current_cycle ^ current_cycle.wrapping_sub(elapsed)) & !0x3FF != 0 {
            self.device_id = hardware.get_device_id();
            self.device_id_pb_bits = (self.device_id & 0x03) << 5;

            let wp = self.mechanics.is_write_protected();
            if wp {
                self.via2.in_b &= !0x10;
            } else {
                self.via2.in_b |= 0x10;
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn poll_mechanics(&mut self, delta: u32, current_cycle: u32) {
        if delta == 0 {
            return;
        }

                // Hardware Wiring: Pull bytes from the DMA buffer and wire SYNC/SO directly to VIA2/CPU.
        if self.is_motor_on() {
            let pcr_cb2 = self.via2.pcr & 0b1110_0000;
            let is_read_mode = pcr_cb2 != 0b1100_0000;

            if is_read_mode {
                let signals = self.mechanics.advance_read_head(delta, current_cycle);
                
                let mut new_byte_arrived = false;
                if let Some(byte) = signals.byte_opt {
                    self.via2.in_a = byte;
                    new_byte_arrived = true;
                    self.time.last_byte_cycle = current_cycle;
                }
                
                // Assert SYNC (Pull PB7 Low). Branchless logic for optimal speed.
                let sync_mask = if signals.is_sync { 0x00 } else { 0x80 };
                self.via2.in_b = (self.via2.in_b & 0x7F) | sync_mask;
                
                if !signals.is_sync && new_byte_arrived {
                    let pcr_ca2 = self.via2.pcr & 0b0000_1110;
                    let is_so_enabled = (pcr_ca2 & 0b0000_1010) == 0b0000_1010;
                    if is_so_enabled {
                        self.via2.ca1_irq();
                        self.so_pulsed = true;
                    }
                }
            } else {
                // WRITE mode
                self.mechanics.advance_write_head(delta, current_cycle, self.via2.ora);
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline]
    pub fn tick_cycles(&mut self, delta: u32) {
        if delta == 0 {
            return;
        }
        // time.last_sync_cycle has already been updated to target_cycle in consume_catchup_delta
        let current_cycle = self.time.last_sync_cycle;
        
        self.poll_mechanics(delta, current_cycle);

        if self.via1.any_active() {
            self.via1.tick_cycles(delta);
        }
        if self.via2.any_active() {
            self.via2.tick_cycles(delta);
        }
    }

    #[link_section = ".data.ram_func"]
    #[cold]
    fn read_io(&mut self, addr: u16, cycle: u32, ctx: &mut H) -> u8 {
        self.catch_up(cycle);

        let result = if (addr & 0x0400) == 0 {
            let reg = (addr & 0x0F) as usize;
            if reg == 0x00 || reg == 0x0F {
                // For reads, adding 1 cycle (1us) delay compensates for the physical RC 
                // rise-time of the open-collector IEC bus. If we sample the instant the C64 
                // releases a line, the voltage might not have risen to logic HIGH yet.
                ctx.spin_until(cycle + 1);
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
                // The physical output pins do not transition until the end of the cycle.
                ctx.spin_until(cycle + 1);
                self.sync_iec_bus(ctx, cycle);
            }
        } else {
            let reg = (addr & 0x0F) as usize;
            self.via2.write(addr, val);
            if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                let effective_orb = self.via2.get_pb_out();

                if effective_orb != self.last_via2_out {
                    ctx.write_mechanics_port(effective_orb);
                    self.last_via2_out = effective_orb;
                }
                self.mechanics.update_state(effective_orb, ctx, cycle);
            }
        }
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
        self.via1.irq || self.via2.irq
    }
}

impl<H: HostInterface, S: MagneticSurface> SystemBus<H, S> {
    #[link_section = ".data.ram_func"]
    #[inline]
    pub fn catch_up(&mut self, target_cycle: u32) {
        let delta = self.time.consume_catchup_delta(target_cycle);
        if delta > 0 {
            self.tick_cycles(delta);
        }
        self.update_next_sync_cycle();
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn update_next_sync_cycle(&mut self) {
        let mut min_delta = 1024 - (self.time.last_sync_cycle & 0x3FF);
        for via in [&self.via1, &self.via2] {
            if via.active_mask & crate::via6522::Via6522::T1_ACTIVE != 0 {
                min_delta = min_delta.min(via.t1c as u32 + 1);
            }
            if via.active_mask & crate::via6522::Via6522::T2_ACTIVE != 0 {
                min_delta = min_delta.min(via.t2c as u32 + 1);
            }
            if via.active_mask & crate::via6522::Via6522::SR_ACTIVE != 0 {
                min_delta = min_delta.min(via.sr_count as u32 + 1);
            }
        }
        
        if self.is_motor_on() {
            // OPTIMIZATION (Option A): We predict the arrival of the next byte from the DMA.
            // A byte at the fastest zone arrives every ~25-26 cycles.
            // By running freely for up to 24 cycles since the last byte, we skip 
            // expensive real-time timer polling until we are in the exact window 
            // where the byte is expected to drop.
            let cycles_since_last_byte = self.time.last_sync_cycle.wrapping_sub(self.time.last_byte_cycle);
            
            // If we are within 24 cycles of the last byte, run freely until we hit the 24 cycle mark.
            // If we exceed 24 cycles, we drop min_delta to 1 to strictly poll real-time every cycle
            // and catch the byte exactly when it arrives (0-jitter).
            let safe_window = 24;
            if cycles_since_last_byte < safe_window {
                let cycles_until_window = safe_window - cycles_since_last_byte;
                min_delta = min_delta.min(cycles_until_window);
            } else {
                min_delta = 1;
            }
        }
        
        self.time.next_event_cycle = self.time.last_sync_cycle.wrapping_add(min_delta);
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
