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
    #[init]
    fn init() {
        pico1541_emulator::init_core1_text();
    }
    use pico1541_common::context::ViaHardware;
    use pico1541_common::interface::{DriveEvent, HostInterface};
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
        fn try_read_byte(&mut self, _delta: u32) -> Option<u8> { self.next_byte }
        fn get_absolute_head_position(&self, _master_clock: u32) -> u32 { 0 }
        fn set_speed_zone(&mut self, _zone: u8) {}
        fn set_motor_running(&mut self, _running: bool, _master_clock: u32) {}
        fn set_stream_source(&mut self, _source: &'static pico1541_common::track_buffer::TrackBuffer, _master_clock: u32) {}
    }

    struct DummyHost;



    impl ViaHardware for DummyHost {
        fn read_iec_port(&mut self) -> u8 { 0xFF }
        fn write_iec_port(&mut self, _via_port_b: u8) {}
        fn write_mechanics_port(&mut self, _via_port_b: u8) {}
        fn consume_trigger_reset(&mut self) -> bool { false }
        fn is_reset_line_asserted(&self) -> bool { false }
    }
impl HostInterface for DummyHost {
        fn send_event(&mut self, _event: DriveEvent) {}
    }

    #[test]
    fn test_mechanics_fast_path() {
        let surface = DummySurface { sync_active: false, next_byte: None };
        static BUFFER: pico1541_common::track_buffer::DualCoreTrackBuffer = pico1541_common::track_buffer::DualCoreTrackBuffer::new();
        let mut mechanics = DriveMechanics::new(surface, BUFFER.split().1);
        let mut host = DummyHost;

        // Setup Motor ON (Bit 2 = 0x04)
        mechanics.update_state(0x04, 0, &mut host, 0);
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                mechanics.advance_read_head(1, 0);
            }
            core::hint::black_box(&mechanics);
        });
        
        defmt::info!("DriveMechanics 1000 tick_cycles(1) fast-path took {} CPU cycles", cycles);
    }

    #[test]
    fn test_mechanics_sync_active() {
        let surface = DummySurface { sync_active: true, next_byte: None };
        static BUFFER: pico1541_common::track_buffer::DualCoreTrackBuffer = pico1541_common::track_buffer::DualCoreTrackBuffer::new();
        let mut mechanics = DriveMechanics::new(surface, BUFFER.split().1);
        let mut host = DummyHost;

        mechanics.update_state(0x04, 0, &mut host, 0);
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                mechanics.advance_read_head(1, 0);
            }
            core::hint::black_box(&mechanics);
        });
        
        defmt::info!("DriveMechanics 1000 tick_cycles(1) sync-active took {} CPU cycles", cycles);
    }

    #[test]
    fn test_mechanics_slow_path() {
        let surface = DummySurface { sync_active: false, next_byte: Some(0x55) };
        static BUFFER: pico1541_common::track_buffer::DualCoreTrackBuffer = pico1541_common::track_buffer::DualCoreTrackBuffer::new();
        let mut mechanics = DriveMechanics::new(surface, BUFFER.split().1);
        let mut host = DummyHost;
        let _via1 = Via6522::new();
        let _via2 = Via6522::new();

        mechanics.update_state(0x04, 0, &mut host, 0);
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                // next_byte is always Some(0x55) because we removed .take() from try_read_byte
                mechanics.advance_read_head(1, 0);
            }
            core::hint::black_box(&mechanics);
        });
        
        defmt::info!("DriveMechanics 1000 tick_cycles(1) slow-path took {} CPU cycles", cycles);
    }
}
