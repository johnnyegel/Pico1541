#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
use embassy_rp as _; // Provides critical-section impl

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
        let prg = pio::pio_asm!(
        "public entry_point:",
        "    set x, 9",
        "",
        "fetch_byte:",
        "    pull block",
        "    out null, 24",
        "",
        "read_bit:",
        "    out y, 1",
        "    jmp !y is_zero",
        "",
        "is_one:",
        "    in y, 1",
        "    jmp x-- check_osre",
        "    ",
        "    ; --- SYNC DETECTED ---",
        "    mov isr, ~null",
        "    push",
        "",
        "sync_wait:",
        "    jmp !osre sync_do",
        "    pull block",
        "    out null, 24",
        "",
        "sync_do:",
        "    out y, 1",
        "    jmp y-- sync_wait",
        "    ",
        "    ; --- SYNC BROKEN ---",
        "    ; Fall through to is_zero, SYNC pin will clear automatically",
        "",
        "is_zero:",
        "    in null, 1",
        "    set x, 9",
        "",
        "check_osre:",
        "    jmp !osre read_bit",
        "    ; OSR is empty -> 8 bits were processed -> A byte is ready!",
        "    push",
        "    jmp fetch_byte",
        );
        
        let loaded_prg = pio.common.load_program(&prg.program);
        
        let sm = &mut pio.sm0;
        let mut cfg = Config::default();
        cfg.shift_in.auto_fill = false;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = ShiftDirection::Left;
        
        cfg.use_program(&loaded_prg, &[]);
        sm.set_config(&cfg);
        
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
        // Push 0xC0 (2 ones, 6 zeros). Total 10 ones -> SYNC.
        sm.tx().push(0x000000C0);
        cortex_m::asm::delay(2000);

        let word1 = sm.rx().try_pull().expect("Expected 0xFF");
        assert_eq!(word1, 0xFF);
        let word2 = sm.rx().try_pull().expect("Expected SYNC 0xFFFFFFFF");
        assert_eq!(word2, 0xFFFFFFFF);
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
        let word2 = sm.rx().try_pull().expect("Expected SYNC 0xFFFFFFFF");
        assert_eq!(word2, 0xFFFFFFFF);
        let word3 = sm.rx().try_pull().expect("Expected 0x01 byte after SYNC");
        assert_eq!(word3, 0x01);

        sm.set_enable(false);
    }
}
