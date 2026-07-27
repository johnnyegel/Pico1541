use crate::drivemechanics::DriveMechanics;
use crate::via6522::Via6522;
#[cfg(feature = "debug")]
use crate::MemoryMonitor;
use core::marker::PhantomData;
#[allow(unused_imports)]
use pico1541_common::context::CatchUpContext;
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::HostInterface;
use pico1541_common::magnetic_surface::MagneticSurface;
use pico1541_common::util::bit_align_u8;

const MEMORY_SIZE: usize = 32768;

/// Represents the 1541's memory bus, mapping access to RAM, ROM, and I/O.
///
/// `H` identifies the concrete `HostInterface` implementation used at the call
/// site.  No instance of `H` is stored here — it is passed into every
/// `read` / `write` call via the `Bus::Ctx` mechanism and forwarded to
/// `tick_cycle`.
pub struct SystemBus<H: HostInterface, S: MagneticSurface> {
    pub memory: alloc::boxed::Box<[u8; MEMORY_SIZE]>,
    pub via1: Via6522,
    pub via2: Via6522,
    pub mechanics: DriveMechanics<S>,
    pub device_id: u8,
    cycle_counter: u32,
    device_id_pb_bits: u8,
    pub last_cycle: u32,
    pub current_cycle: u32,
    pub next_sync_cycle: u32,
    pub sync_required: bool,
    pub so_pulsed: bool,
    pub iec_synced_this_instruction: bool,
    last_iec_out: u8,
    last_via2_out: u8,
    _phantom: PhantomData<H>,
}

impl<H: HostInterface, S: MagneticSurface> SystemBus<H, S> {
    pub fn new(rom: &'static [u8; 16384], device_id: u8, mechanics: DriveMechanics<S>) -> Self {
        // :: Intialize VIA1
        let via1 = Via6522::new();

        let mem_ptr = unsafe {
            alloc::alloc::alloc_zeroed(core::alloc::Layout::new::<[u8; MEMORY_SIZE]>())
                as *mut [u8; MEMORY_SIZE]
        };
        let mut memory = unsafe { alloc::boxed::Box::from_raw(mem_ptr) };
        memory[0x4000..0x8000].copy_from_slice(rom);

        let mut systembus = Self {
            memory,
            via1,
            via2: Via6522::new(),
            mechanics,
            device_id,
            cycle_counter: 0,
            device_id_pb_bits: (device_id & 0x03) << 5,
            last_cycle: 0,
            current_cycle: 0,
            next_sync_cycle: 1024,
            sync_required: false,
            so_pulsed: false,
            iec_synced_this_instruction: false,
            last_iec_out: 0xFF, // Force first write
            last_via2_out: 0xFF,
            _phantom: PhantomData,
        };

        // Set up the VIA1 initial state
        systembus.via1.in_a = 0x0;
        systembus.via1.in_b = 0x0;

        systembus
    }

    pub fn update_rom(&mut self, rom: &'static [u8; 16384]) {
        self.memory[0x4000..0x8000].copy_from_slice(rom);
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
        self.memory[0..0x0800].fill(0);
        self.via1.reset();
        self.via2.reset();
        self.mechanics.reset();
        self.device_id_pb_bits = (self.device_id & 0x03) << 5;
        self.last_iec_out = 0xFF; // Force first write
        self.last_via2_out = 0xFF;
        self.mechanics
            .update_state(self.via2.get_pb_out(), hardware);
        // Force the track manager to load the reset track
        hardware.track_changed(self.mechanics.half_track());
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn sync_iec_bus(&mut self, hardware: &mut H) {
        // :: Capture the IEC bus state, and update the VIA1 input register
        let iec_in = hardware.read_iec_port() | self.device_id_pb_bits;
        let prev_atn = self.via1.in_b & H::VIA1_IEC_MASK_ATN_RX;
        let curr_atn = iec_in & H::VIA1_IEC_MASK_ATN_RX;
        self.via1.in_b = iec_in;

        // :? Did the ATN state change?
        if prev_atn != curr_atn {
            // -> Yes, then call the VIA1 CA1 edge handler to trigger an interrupt if needed
            self.via1.ca1_edge(curr_atn != 0);
            self.sync_required = true;
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
        
        self.iec_synced_this_instruction = true;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn poll_realtime(&mut self, hardware: &mut H, elapsed: u32) {
        if !self.iec_synced_this_instruction {
            self.sync_iec_bus(hardware);
        }

        hardware.step();

        let old_cc = self.cycle_counter;
        self.cycle_counter = self.cycle_counter.wrapping_add(elapsed);
        if (old_cc ^ self.cycle_counter) & !0x3FF != 0 {
            self.device_id = hardware.get_device_id();
            self.device_id_pb_bits = (self.device_id & 0x03) << 5;

            if hardware.pop_discard_request() {
                crate::hle::HleManager::intercept_command(self);
                // Also write iec port here?
            }
            // FIXME! What the actual fuck is this?
            if self.mechanics.pop_force_disk_change() {
                self.memory[0x001C] = 1;
            }

            let wp = self.mechanics.is_write_protected();
            if wp {
                self.via2.in_b &= !0x10;
            } else {
                self.via2.in_b |= 0x10;
            }
        }

        // Hardware wiring and VIA tick logic is now handled in catch_up,
        // which guarantees it executes precisely mid-instruction if an I/O read/write occurs!
        // We no longer tick mechanics here.
    }

    #[link_section = ".data.ram_func"]
    #[inline]
    pub fn tick_cycles(&mut self, delta: u32) {
        if delta == 0 {
            return;
        }
        
        self.mechanics.tick();
        
        if self.via1.any_active() {
            self.via1.tick_cycles(delta);
        }
        if self.via2.any_active() {
            self.via2.tick_cycles(delta);
        }

        // Hardware Wiring: Pull bytes from the DMA buffer and wire SYNC/SO directly to VIA2/CPU.
        let is_motor_on = (self.via2.orb | !self.via2.ddrb) & 0x04 != 0;
        if is_motor_on {
            let pcr_cb2 = self.via2.pcr & 0b1110_0000;
            let is_read_mode = pcr_cb2 != 0b1100_0000;

            if is_read_mode {
                let byte_opt = self.mechanics.surface.try_read_byte(delta);
                let is_sync = self.mechanics.surface.is_sync_active();
                let was_sync = (self.via2.in_b & 0x80) == 0;
                
                let mut new_byte_arrived = false;
                if let Some(byte) = byte_opt {
                    self.via2.in_a = byte;
                    new_byte_arrived = true;
                }
                
                if is_sync != was_sync {
                    if is_sync {
                        self.via2.in_b &= !0x80; // Assert SYNC (Pull PB7 Low)
                    } else {
                        self.via2.in_b |= 0x80;  // Release SYNC
                    }
                }
                
                if !is_sync && new_byte_arrived {
                    let pcr_ca2 = self.via2.pcr & 0b0000_1110;
                    let is_so_enabled = (pcr_ca2 & 0b0000_1010) == 0b0000_1010;
                    if is_so_enabled {
                        self.via2.ca1_irq();
                        self.so_pulsed = true;
                    }
                }
            } else {
                // WRITE mode
                let index = self.mechanics.surface.get_absolute_head_position(0);
                self.mechanics.write_byte_at(index, self.via2.ora);
            }
        }
    }

    #[link_section = ".data.ram_func"]
    #[cold]
    fn read_io(&mut self, addr: u16, cycle: u32, _ctx: &mut H) -> u8 {
        self.catch_up(cycle);
        self.sync_required = true;

        // Spinlock to exact cycle before reading live IEC state
        _ctx.spin_until(cycle + 1);

        let result = if (addr & 0x0400) == 0 {
            if (addr & 0x0F) == 0x00 {
                _ctx.debug_pin_0(true);
            }
            let reg = (addr & 0x0F) as usize;
            if reg == 0x00 || reg == 0x0F {
                self.sync_iec_bus(_ctx);
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
        self.sync_required = true;

        // Spinlock to exact cycle before writing to physical pins
        ctx.spin_until(cycle);

        if (addr & 0x0400) == 0 {
            if (addr & 0x0F) == 0x00 {
                ctx.debug_pin_0(true);
            }
            let reg = (addr & 0x0F) as usize;
            self.via1.write(addr, val);
            if reg == 0x00 || reg == 0x02 || reg == 0x0F {
                self.sync_iec_bus(ctx);
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
                self.mechanics.update_state(effective_orb, ctx);
            }
        }
    }
}

impl<H: HostInterface, S: MagneticSurface> CatchUpContext for SystemBus<H, S> {
    fn catch_up(&mut self, target_cycle: u64) {
        self.catch_up(target_cycle as u32);
    }
    fn advance_to(&mut self, target_cycle: u64) {
        self.last_cycle = target_cycle as u32;
    }
}

impl<H: HostInterface, S: MagneticSurface> racc6502v4::bus::Bus for SystemBus<H, S> {
    type Ctx = H;

    #[inline(always)]
    fn get_instruction_ptr(&self, addr: u16) -> *const u8 {
        if addr < 0x8000 {
            unsafe { self.memory.as_ptr().add((addr & 0x07FF) as usize) }
        } else {
            let index = (addr & 0x3FFF) | ((addr >> 1) & 0x4000);
            unsafe { self.memory.as_ptr().add(index as usize) }
        }
    }

    #[inline(always)]
    fn read(&mut self, addr: u16, cycle: u32, hw: &mut H) -> u8 {
        if (addr & 0x9800) == 0x1800 {
            self.read_io(addr, cycle, hw)
        } else if addr < 0x8000 {
            let index = addr & 0x07FF;
            unsafe { *self.memory.get_unchecked(index as usize) }
        } else {
            let index = (addr & 0x3FFF) | ((addr >> 1) & 0x4000);
            unsafe { *self.memory.get_unchecked(index as usize) }
        }
    }

    #[inline(always)]
    fn write(&mut self, addr: u16, val: u8, cycle: u32, ctx: &mut Self::Ctx) {
        if (addr & 0x9800) == 0x1800 {
            self.write_io(addr, val, cycle, ctx);
        } else if addr < 0x8000 {
            let index = addr & 0x07FF;
            unsafe {
                *self.memory.get_unchecked_mut(index as usize) = val;
            }
            let mapped = index;
            if mapped == 0x0077 || mapped == 0x0078 {
                ctx.update_iec_addresses(self.memory[0x0077], self.memory[0x0078]);
            }
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
        let delta = target_cycle.wrapping_sub(self.last_cycle);
        if delta > 0 && delta < 0x80000000 {
            self.tick_cycles(delta);
            self.last_cycle = target_cycle;
        }
        self.update_next_sync_cycle();
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn update_next_sync_cycle(&mut self) {
        let mut min_delta = 1024 - (self.cycle_counter & 0x3FF);
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
        
        let is_motor_on = (self.via2.orb | !self.via2.ddrb) & 0x04 != 0;
        if is_motor_on {
            min_delta = 1;
        }

        self.next_sync_cycle = self.last_cycle.wrapping_add(min_delta);
    }

    
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn so_pending(&mut self) -> bool {
        let result = self.so_pulsed;
        self.so_pulsed = false;
        result
    }
}

/// Implement MemoryMonitor for SystemBus to allow peeking at memory without affecting timing.
#[cfg(feature = "debug")]
impl<H: HostInterface, S: MagneticSurface> MemoryMonitor for SystemBus<H, S> {
    fn peek<H2: ViaHardware, C2: CatchUpContext>(
        &mut self,
        addr: u16,
        hw: &mut H2,
        ctx: &mut C2,
    ) -> u8 {
        if (addr & 0x9800) == 0x1800 {
            if addr < 0x1C00 {
                self.via1.peek(addr, hw, ctx)
            } else {
                self.via2.peek(addr, hw, ctx)
            }
        } else {
            let index = (addr & 0x3FFF) | ((addr >> 1) & 0x4000);
            self.memory[index as usize]
        }
    }

    fn poke<H2: ViaHardware, C2: CatchUpContext>(
        &mut self,
        _addr: u16,
        _value: u8,
        _hw: &mut H2,
        _ctx: &mut C2,
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
        let mut bus = SystemBus::<MockHardware, _>::new(&ROM, 8, mechanics);
        let mut hw = MockHardware {};
        use racc6502v4::bus::Bus;
        bus.write(0x0100, 0xAB, 0, &mut hw);
        assert_eq!(bus.read(0x0100, 0, &mut hw), 0xAB);
    }
}
