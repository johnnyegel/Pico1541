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
    fn init() -> (Pio<'static, PIO0>, embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_0>, embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_1>) {
        let p = embassy_rp::init(Default::default());
        (Pio::new(p.PIO0, Irqs), p.PIN_0, p.PIN_1)
    }

    #[test]
    fn test_pio_logic(state: &mut (Pio<'static, PIO0>, embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_0>, embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_1>)) {
        let prg = pio::pio_asm!(
        ".side_set 2 opt",
        "public entry_point:",
        "    set x, 9           side 0b00  ; Clear SYNC and SO",
        "",
        "fetch_byte:",
        "    pull block         side 0b00  ; Fetch 32-bit word from DMA (bottom 8 bits contain data)",
        "    out null, 24       side 0b00  ; Discard top 24 zeros",
        "",
        "read_bit:",
        "    out y, 1           side 0b00  ; Shift 1 bit of real data into Y",
        "    jmp !y is_zero     side 0b00  ; If 0, handle alignment",
        "",
        "is_one:",
        "    in y, 1            side 0b00  ; Shift 1 into ISR",
        "    jmp x-- check_osre side 0b00  ; If we haven't hit 10 ones yet, loop",
        "    ",
        "    ; --- SYNC DETECTED ---",
        "    mov isr, ~null     side 0b10  ; Fill ISR with all 1s",
        "    push               side 0b10  ; Assert SYNC (Pin 1 HIGH), push 0xFFFFFFFF",
        "",
        "sync_wait:",
        "    jmp !osre sync_do  side 0b10",
        "    pull block         side 0b10",
        "    out null, 24       side 0b10",
        "",
        "sync_do:",
        "    out y, 1           side 0b10",
        "    jmp y-- sync_wait  side 0b10  ; If still 1, stay in SYNC",
        "    ",
        "    ; --- SYNC BROKEN ---",
        "    ; Fall through to is_zero, SYNC pin will clear automatically",
        "",
        "is_zero:",
        "    in null, 1         side 0b00  ; Shift 0 into ISR, CLEAR SYNC!",
        "    set x, 9           side 0b00  ; Reset SYNC counter",
        "",
        "check_osre:",
        "    jmp !osre read_bit side 0b00",
        "    ; OSR is empty -> 8 bits were processed -> A byte is ready!",
        "    push               side 0b01  ; Assert SO pulse (Pin 0 HIGH) and push byte to RX FIFO",
        "    jmp fetch_byte     side 0b01  ; Fetch next byte",
        );
        
        let pio = &mut state.0;
        let loaded_prg = pio.common.load_program(&prg.program);
        
        let sm = &mut pio.sm0;
        let mut cfg = Config::default();
        cfg.shift_in.auto_fill = false;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = ShiftDirection::Left;
        
        let pin0 = pio.common.make_pio_pin(unsafe { core::ptr::read(&state.1 as *const _) });
        let pin1 = pio.common.make_pio_pin(unsafe { core::ptr::read(&state.2 as *const _) });
        cfg.use_program(&loaded_prg, &[&pin0, &pin1]);
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
