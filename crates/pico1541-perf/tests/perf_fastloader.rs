#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_rp as _;

#[global_allocator]
static HEAP: embedded_alloc::Heap = embedded_alloc::Heap::empty();
#[defmt_test::tests]
mod tests {
    use pico1541_emulator::Emulator;
    use pico1541_common::interface::{DriveEvent, HostInterface, IecBusState};
    use pico1541_common::magnetic_surface::MagneticSurface;

    struct DummyHost;

    impl pico1541_common::interface::IecBus for DummyHost {
        fn iec_host(&self) -> IecBusState { IecBusState::default() }
        fn iec_drive(&mut self, _state: IecBusState) {}
    }

    impl pico1541_common::context::ViaHardware for DummyHost {}
impl HostInterface for DummyHost {
        fn sync(&mut self, _host_state: IecBusState) {}
        fn step(&mut self) {}
        fn get_device_id(&self) -> u8 { 8 }
        fn pop_discard_request(&mut self) -> bool { false }
        fn check_halt_requested(&self) -> bool { false }
        fn consume_trigger_reset(&mut self) -> bool { false }
        fn is_reset_line_asserted(&self) -> bool { false }
        fn track_changed(&mut self, _half_track: u8) {}
        fn debug_pin_1(&mut self, _on: bool) {}
        fn send_event(&mut self, _event: DriveEvent) {}
        fn update_iec_addresses(&mut self, _listen: u8, _talk: u8) {}
    }

    struct DummySurface {
        cycles_since_byte: u32,
    }

    impl MagneticSurface for DummySurface {
        fn pop_force_disk_change(&mut self) -> bool { false }
        fn is_write_protected(&self) -> bool { true }
        fn try_read_byte(&mut self, delta_cycles: u32) -> Option<u8> {
            self.cycles_since_byte += delta_cycles;
            if self.cycles_since_byte >= 26 {
                self.cycles_since_byte -= 26;
                Some(0x55) // Dummy data
            } else {
                None
            }
        }
        fn is_sync_active(&self) -> bool { false }
        fn get_absolute_head_position(&self, _master_clock: u32) -> u32 { 0 }
        fn write_byte_at(&mut self, _index: u32, _data: u8) {}
        fn start_reading(&mut self, _track_id: u8, _rotation_fixed: u32) {}
        fn stop_reading(&mut self) {}
        fn set_speed_zone(&mut self, _zone: u8) {}
        fn inject_analog_noise(&mut self) {}
        fn get_active_speed_zone(&self) -> Option<u8> { Some(0) }
        fn get_active_track_length(&self) -> usize { 0 }
    }

    static ROM: [u8; 16384] = {
        let mut rom = [0u8; 16384];
        
        // Fastloader loop at ROM base $C000 (Index 0 in our ROM buffer)
        // Wait for SO pulse: BVC (0x50) 0xFE (jump back to itself)
        rom[0] = 0x50; rom[1] = 0xFE; 
        
        // CLV (0xB8)
        rom[2] = 0xB8; 
        
        // LDA $1C01 (0xAD 0x01 0x1C)
        rom[3] = 0xAD; rom[4] = 0x01; rom[5] = 0x1C;
        
        // STA $1800 (0x8D 0x00 0x18)
        rom[6] = 0x8D; rom[7] = 0x00; rom[8] = 0x18;
        
        // JMP $C000 (0x4C 0x00 0xC0)
        rom[9] = 0x4C; rom[10] = 0x00; rom[11] = 0xC0;

        // Reset Vector at $FFFC-$FFFD (Index 0x3FFC in buffer)
        rom[0x3FFC] = 0x00;
        rom[0x3FFD] = 0xC0;
        
        rom
    };

    macro_rules! measure_cycles {
        ($block:expr) => {{
            use cortex_m::peripheral::SYST;
            let mut syst = unsafe { cortex_m::Peripherals::steal().SYST };
            syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
            syst.set_reload(0x00FFFFFF);
            syst.clear_current();
            syst.enable_counter();

            let start = SYST::get_current();
            $block
            let end = SYST::get_current();
            syst.disable_counter();
            start.wrapping_sub(end) & 0x00FFFFFF
        }};
    }

    #[test]
    fn test_fastloader_stress() {
        {
            use core::mem::MaybeUninit;
            const HEAP_SIZE: usize = 32768 + 1024;
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            // Only init if not already initialized
            unsafe { if super::HEAP.free() == 0 { super::HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) } }
        }

        let surface = DummySurface { cycles_since_byte: 0 };
        let mut emulator = Emulator::new(&ROM, 8, surface);
        let mut host = DummyHost;

        emulator.reset(&mut host);

        // Turn on motor to ensure DriveMechanics operates properly and generates SO pulses
        emulator.memory.via2.orb |= 0x04;
        emulator.memory.mechanics.update_state(emulator.memory.via2.orb, emulator.memory.via2.ddrb, &mut host);

        // Pre-run a few cycles to get the CPU past the reset vector logic and into our loop
        emulator.run_for(&mut host, 100);
        
        // Sync our cycle counter reference
        let test_start_cycles = emulator.cycles;
        let mut total_loops = 0;
        
        let cycles = measure_cycles!({
            // Run exactly 1,000,000 emulated cycles (1 real second)
            while emulator.cycles - test_start_cycles < 1_000_000 {
                emulator.tick(&mut host);
                total_loops += 1;
            }
            core::hint::black_box(&emulator);
        });

        defmt::info!("Emulated 1,000,000 cycles in {} ARM CPU cycles", cycles);
        defmt::info!("Total CPU instructions (ticks) executed: {}", total_loops);
    }

    static ROM_2BIT: [u8; 16384] = {
        let mut rom = [0u8; 16384];
        
        // Fastloader 2-bit unrolled loop at ROM base $C000
        rom[0] = 0x50; rom[1] = 0xFE;  // BVC *-2
        rom[2] = 0xB8;                 // CLV
        rom[3] = 0xAD; rom[4] = 0x01; rom[5] = 0x1C; // LDA $1C01
        
        // Unrolled 2-bit transmission
        rom[6] = 0x8D; rom[7] = 0x00; rom[8] = 0x18; // STA $1800
        rom[9] = 0x4A; // LSR A
        rom[10] = 0x4A; // LSR A
        rom[11] = 0x8D; rom[12] = 0x00; rom[13] = 0x18; // STA $1800
        rom[14] = 0x4A; // LSR A
        rom[15] = 0x4A; // LSR A
        rom[16] = 0x8D; rom[17] = 0x00; rom[18] = 0x18; // STA $1800
        rom[19] = 0x4A; // LSR A
        rom[20] = 0x4A; // LSR A
        rom[21] = 0x8D; rom[22] = 0x00; rom[23] = 0x18; // STA $1800
        
        rom[24] = 0x4C; rom[25] = 0x00; rom[26] = 0xC0; // JMP $C000

        // Reset Vector
        rom[0x3FFC] = 0x00;
        rom[0x3FFD] = 0xC0;
        
        rom
    };

    #[test]
    fn test_fastloader_2bit_unrolled() {
        {
            use core::mem::MaybeUninit;
            const HEAP_SIZE: usize = 32768 + 1024;
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { if super::HEAP.free() == 0 { super::HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) } }
        }

        let surface = DummySurface { cycles_since_byte: 0 };
        let mut emulator = Emulator::new(&ROM_2BIT, 8, surface);
        let mut host = DummyHost;

        emulator.reset(&mut host);
        emulator.memory.via2.orb |= 0x04;
        emulator.memory.mechanics.update_state(emulator.memory.via2.orb, emulator.memory.via2.ddrb, &mut host);
        emulator.run_for(&mut host, 100);
        
        let test_start_cycles = emulator.cycles;
        let mut total_loops = 0;
        
        let cycles = measure_cycles!({
            while emulator.cycles - test_start_cycles < 1_000_000 {
                emulator.tick(&mut host);
                total_loops += 1;
            }
            core::hint::black_box(&emulator);
        });

        defmt::info!("2-BIT UNROLLED: Emulated 1,000,000 cycles in {} ARM CPU cycles", cycles);
        defmt::info!("2-BIT UNROLLED: Total CPU instructions executed: {}", total_loops);
    }
}
