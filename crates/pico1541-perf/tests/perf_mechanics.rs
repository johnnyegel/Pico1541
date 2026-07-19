#![no_std]
#![no_main]

#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use defmt_rtt as _;
use panic_probe as _;
use embassy_rp as _;

#[global_allocator]
static ALLOCATOR: DummyAlloc = DummyAlloc;

struct DummyAlloc;

unsafe impl core::alloc::GlobalAlloc for DummyAlloc {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 { core::ptr::null_mut() }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[defmt_test::tests]
mod tests {
    use pico1541_common::interface::{DriveEvent, HostInterface, IecBus, IecBusState};
    use pico1541_common::magnetic_surface::MagneticSurface;
    use pico1541_emulator::drivemechanics::DriveMechanics;
    use pico1541_emulator::via6522::Via6522;
    use cortex_m::peripheral::SYST;

    macro_rules! measure_cycles {
        ($block:expr) => {{
            let mut syst = unsafe { cortex_m::Peripherals::steal().SYST };
            syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
            syst.set_reload(0x00FFFFFF);
            syst.clear_current();
            syst.enable_counter();
            let start = SYST::get_current();
            
            $block
            
            let end = SYST::get_current();
            syst.disable_counter();
            (start - end) & 0x00FFFFFF
        }};
    }

    struct DummySurface {
        pub sync_active: bool,
        pub next_byte: Option<u8>,
    }

    impl MagneticSurface for DummySurface {
        fn is_sync_active(&self) -> bool { self.sync_active }
        fn try_read_byte(&mut self, _delta: u32) -> Option<u8> { self.next_byte.take() }
        fn get_absolute_head_position(&self, _master_clock: u32) -> u32 { 0 }
        fn write_byte_at(&mut self, _index: u32, _data: u8) {}
        fn start_reading(&mut self, _track_id: u8, _rotation_fixed: u32) {}
        fn stop_reading(&mut self) {}
        fn is_write_protected(&self) -> bool { true }
        fn pop_force_disk_change(&mut self) -> bool { false }
        fn set_speed_zone(&mut self, _zone: u8) {}
        fn inject_analog_noise(&mut self) {}
        fn get_active_speed_zone(&self) -> Option<u8> { Some(0) }
        fn get_active_track_length(&self) -> usize { 0 }
    }

    struct DummyHost;

    impl IecBus for DummyHost {
        fn iec_host(&self) -> IecBusState { IecBusState::default() }
        fn iec_drive(&mut self, _state: IecBusState) {}
    }

    impl ViaHardware for DummyHost {}
impl HostInterface for DummyHost {
        fn send_event(&mut self, _event: DriveEvent) {}
    }

    #[test]
    fn test_mechanics_fast_path() {
        let surface = DummySurface { sync_active: false, next_byte: None };
        let mut mechanics = DriveMechanics::new(surface);
        let mut host = DummyHost;
        let mut via1 = Via6522::new();
        let mut via2 = Via6522::new();

        // Setup Motor ON (Bit 2 = 0x04)
        mechanics.update_state(0x04, 0x04, &mut host);
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                mechanics.tick_cycles(&mut host, &mut via1, &mut via2, 1);
            }
            core::hint::black_box(&mechanics);
        });
        
        defmt::info!("DriveMechanics 1000 tick_cycles(1) fast-path took {} CPU cycles", cycles);
    }



    #[test]
    fn test_mechanics_slow_path() {
        let surface = DummySurface { sync_active: false, next_byte: Some(0x55) };
        let mut mechanics = DriveMechanics::new(surface);
        let mut host = DummyHost;
        let mut via1 = Via6522::new();
        let mut via2 = Via6522::new();

        mechanics.update_state(0x04, 0x04, &mut host);
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                // Ensure there is always a byte available to force slow path
                unsafe { core::ptr::write_volatile(&mut mechanics.surface.next_byte, Some(0x55)) };
                mechanics.tick_cycles(&mut host, &mut via1, &mut via2, 1);
            }
            core::hint::black_box(&mechanics);
        });
        
        defmt::info!("DriveMechanics 1000 tick_cycles(1) slow-path took {} CPU cycles", cycles);
    }
}
