#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_rp as _; // Provides critical-section impl

#[cfg(not(test))]
#[cortex_m_rt::entry]
fn main() -> ! {
    loop {}
}

pub mod pio {
    pub fn magnetic_surface_program() -> ::pio::Program<32> {
        ::pio::pio_file!("src/pio/magnetic_surface.pio").program
    }
}
#[path = "../magnetic_surface.rs"]
pub mod magnetic_surface;


#[defmt_test::tests]
mod tests {
    use defmt::{assert, info};
    use embassy_rp::pio::Pio;
    use embassy_rp::peripherals::PIO0;
    use embassy_rp::bind_interrupts;
    use pico1541_common::track_buffer::{DualCoreTrackBuffer, TrackLayout};
    use pico1541_common::magnetic_surface::MagneticSurface;
    use super::magnetic_surface::PioMagneticSurface;
    use embassy_time::Instant;

    include!("../../../../temp/track_18_data.rs"); // gives TRACK_18_DATA

    bind_interrupts!(struct Irqs {
        PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    });

    #[init]
    fn init() {
        let mut config = embassy_rp::config::Config::default();
        config.clocks = embassy_rp::clocks::ClockConfig::system_freq(pico1541_common::SYSTEM_CLOCK_FREQ).unwrap();
        embassy_rp::init(config);
    }

    #[test]
    fn test_track_streaming() {
        let pio_block = unsafe { embassy_rp::peripherals::PIO0::steal() };
        let pio = Pio::new(pio_block, Irqs);

        info!("Allocating track buffer...");
        // Using static to avoid stack overflow
        static mut BUFFER: DualCoreTrackBuffer = DualCoreTrackBuffer::new();
        let dual_buffer = unsafe { core::ptr::addr_of_mut!(BUFFER).as_mut().unwrap() };
        let (manager_handle, emulator_handle) = dual_buffer.split();

        info!("Loading track data ({} bytes)...", TRACK_18_DATA.len());
        let active_buf = manager_handle.start_track_modification();
        active_buf[..TRACK_18_DATA.len()].copy_from_slice(TRACK_18_DATA);
        
        manager_handle.commit_loaded_track(34, TrackLayout::Accurate, TRACK_18_DATA.len(), Some(2));
        
        // Spin up PIO and DMA
        let mut surface = PioMagneticSurface::new(pio, emulator_handle);
        surface.start_reading(34, 0);
        surface.set_speed_zone(2);

        let mut byte_count = 0;
        let mut sync_count = 0;
        let start_time = Instant::now();
        let mut was_sync = false;

        // Track 18 at 288 RPM, bitrate zone 2 is ~284,000 bits/sec
        // So a full rotation takes exactly 200,000 microseconds (0.2s = 5 rotations/sec).
        // Let's read for 400,000 us (2 full rotations)
        while start_time.elapsed().as_micros() < 400_000 {
            if let Some(_byte) = surface.try_read_byte(0) {
                byte_count += 1;
            }

            // Check sync state changes
            let is_sync = PioMagneticSurface::is_sync_active();
            if is_sync && !was_sync {
                sync_count += 1;
            }
            was_sync = is_sync;
        }

        surface.stop_reading();

        info!("Finished reading. Extracted {} bytes and found {} sync marks.", byte_count, sync_count);

        assert!(byte_count > 13000 && byte_count < 15500, "Byte rate is incorrect!");
        assert!(sync_count > 30, "Should have found >30 SYNC marks!");
        assert!(byte_count > 12000, "Should have extracted >12000 bytes!");
    }
}
