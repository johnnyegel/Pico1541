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

#[defmt_test::tests]
mod tests {
    use defmt::{assert, assert_eq, info};
    use embassy_rp::pio::{Pio, ShiftDirection, Config};
    use embassy_rp::peripherals::PIO0;
    use embassy_rp::bind_interrupts;

    bind_interrupts!(struct Irqs {
        PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    });

    #[init]
    fn init() -> Pio<'static, PIO0> {
        let p = embassy_rp::init(Default::default());
        Pio::new(p.PIO0, Irqs)
    }

    #[test]
    fn test_pio_logic(pio: &mut Pio<'static, PIO0>) {
        let prg = pio::pio_file!("src/pio/magnetic_surface.pio").program;
        let loaded_prg = pio.common.load_program(&prg);
        
        let sm = &mut pio.sm0;
        let mut cfg = Config::default();
        cfg.shift_in.auto_fill = true;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = ShiftDirection::Left;
        
        // Map SET pins to GPIO 10 to test SYNC
        let sync_pin = pio.common.make_pio_pin(unsafe { embassy_rp::peripherals::PIN_10::steal() });
        cfg.set_set_pins(&[&sync_pin]);

        cfg.use_program(&loaded_prg, &[]);
        sm.set_config(&cfg);
        sm.set_pin_dirs(embassy_rp::pio::Direction::Out, &[&sync_pin]);
        
        // --- PHASE 1: Normal bytes test ---
        info!("Phase 1: Normal bytes test");
        sm.clear_fifos();
        sm.set_enable(true);
        sm.tx().push(0x00000055); // 01010101
        sm.tx().push(0x000000AA); // 10101010
        cortex_m::asm::delay(2000);
        let word1 = sm.rx().try_pull().expect("Expected 0x55");
        assert_eq!(word1, 0x55);
        let word2 = sm.rx().try_pull().expect("Expected 0xAA");
        assert_eq!(word2, 0xAA);
        assert!(sm.rx().try_pull().is_none());
        sm.set_enable(false);

        // --- PHASE 2: SYNC detection test ---
        info!("Phase 2: SYNC detection test");
        sm.clear_fifos();
        // Clear internal registers by forcing execution of clear instructions or restart
        embassy_rp::pac::PIO0.ctrl().write(|w| {
            w.set_sm_restart(1 << 0);
            w.set_sm_enable(1 << 0);
        });
        
        // Push 0xFF (8 ones)
        sm.tx().push(0x000000FF);
        // Push 0xFF (8 ones). Total 16 ones -> SYNC stays active.
        sm.tx().push(0x000000FF);
        cortex_m::asm::delay(2000);

        let word1 = sm.rx().try_pull().expect("Expected 0xFF");
        assert_eq!(word1, 0xFF);
        
        // Wait, does it push the 10th one into RX?
        // "mov isr, null" purges the partial ISR buffer. So we expect NOTHING else!
        // But the SYNC pin should be HIGH!
        let sio = embassy_rp::pac::SIO;
        let is_sync = (sio.gpio_in(0).read() & (1 << 10)) != 0;
        assert!(is_sync, "Expected SYNC pin (GPIO 10) to be HIGH");
        
        // Now break sync by pushing a 0
        sm.tx().push(0x00000000);
        cortex_m::asm::delay(2000);
        
        // The 0 byte after sync is broken should be pushed
        let word3 = sm.rx().try_pull().expect("Expected remaining 0x00 byte");
        assert_eq!(word3, 0x00);
        assert!(sm.rx().try_pull().is_none());
        sm.set_enable(false);

        // --- PHASE 3: Long SYNC test ---
        info!("Phase 3: Long SYNC test");
        sm.clear_fifos();
        embassy_rp::pac::PIO0.ctrl().write(|w| {
            w.set_sm_restart(1 << 0);
            w.set_sm_enable(1 << 0);
        });
        
        // Push 32 ones (0xFF, 0xFF, 0xFF, 0xFF)
        sm.tx().push(0x000000FF);
        sm.tx().push(0x000000FF);
        sm.tx().push(0x000000FF);
        sm.tx().push(0x000000FF);
        // Then break it with 0x01
        sm.tx().push(0x00000001);
        cortex_m::asm::delay(4000);

        let word1 = sm.rx().try_pull().expect("Expected 0xFF from first 8 bits of SYNC");
        assert_eq!(word1, 0xFF);
        
        let is_sync = (sio.gpio_in(0).read() & (1 << 10)) != 0;
        // Since we pushed 0x01 at the end, SYNC is broken by the end of the delay!
        assert!(!is_sync, "Expected SYNC pin (GPIO 10) to be LOW after sync broken");
        
        let word3 = sm.rx().try_pull().expect("Expected 0x01 byte after SYNC");
        assert_eq!(word3, 0x01);

        sm.set_enable(false);
    }
}
