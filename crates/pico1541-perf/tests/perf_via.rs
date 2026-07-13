#![no_std]
#![no_main]

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
    use pico1541_emulator::via6522::Via6522;
    use cortex_m::peripheral::SYST;

    // We can use a simple macro to measure systick difference
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

    #[test]
    fn test_via6522_tick() {
        let mut via = Via6522::new();
        
        // Setup VIA to actually do some work
        via.write(0x04, 0xFF); // T1L-L
        via.write(0x05, 0xFF); // T1C-H (Starts T1)
        
        let cycles = measure_cycles!({
            for _ in 0..1000 {
                via.tick_cycles(1);
            }
            core::hint::black_box(&via);
        });
        
        defmt::info!("Via6522 1000 tick_cycles(1) took {} CPU cycles", cycles);
    }
    
    #[test]
    fn test_via6522_read_write() {
        let mut via = Via6522::new();
        
        let cycles = measure_cycles!({
            for i in 0..1000 {
                via.write(i % 16, i as u8);
                let val = via.read(i % 16);
                core::hint::black_box(val);
            }
        });
        
        defmt::info!("Via6522 1000 read/write operations took {} CPU cycles", cycles);
    }
}
