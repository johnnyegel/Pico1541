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
    fn test_pio_sync_detection(pio: &mut Pio<'static, PIO0>) {
        let prg = pio::pio_asm!(
            ".side_set 2 opt",
            "public entry_point:",
            "    set x, 9           side 0b00",
            "check_osre:",
            "    jmp !osre read_bit side 0b00",
            "    jmp fetch_byte     side 0b01",
            "fetch_byte:",
            "    pull block         side 0b00",
            "    out null, 24       side 0b00",
            "read_bit:",
            "    out y, 1           side 0b00",
            "    jmp !y is_zero     side 0b00",
            "is_one:",
            "    in y, 1            side 0b00",
            "    jmp x-- check_osre side 0b00",
            "    push               side 0b10",
            "sync_wait:",
            "    jmp !osre sync_do  side 0b10",
            "    pull block         side 0b10",
            "    out null, 24       side 0b10",
            "sync_do:",
            "    out y, 1           side 0b10",
            "    jmp y-- sync_wait  side 0b10",
            "is_zero:",
            "    in null, 1         side 0b00",
            "    set x, 9           side 0b00",
            "    jmp check_osre     side 0b00"
        );
        let loaded_prg = pio.common.load_program(&prg.program);
        
        let mut sm = pio.sm0;
        let mut cfg = Config::default();
        cfg.shift_in.auto_fill = true;
        cfg.shift_in.threshold = 8;
        cfg.shift_in.direction = ShiftDirection::Left;
        
        cfg.shift_out.auto_fill = false;
        cfg.shift_out.threshold = 32;
        cfg.shift_out.direction = ShiftDirection::Left;
        
        cfg.use_program(&loaded_prg, &[]);
        sm.set_config(&cfg);
        
        sm.clear_fifos();
        sm.set_enable(true);
        
        // Push 0xFF (8 ones)
        sm.tx().push(0x000000FF);
        // Push 0xC0 (2 ones, 6 zeros)
        sm.tx().push(0x000000C0);
        
        let mut timeout = 1000;
        while timeout > 0 {
            cortex_m::asm::delay(100);
            timeout -= 1;
        }

        let mut rx_count = 0;
        while let Some(word) = sm.rx().try_pull() {
            info!("RX FIFO: {:X}", word);
            rx_count += 1;
            if rx_count == 1 {
                assert_eq!(word, 0xFF);
            }
        }
        assert!(rx_count > 0, "No bytes received!");
        sm.set_enable(false);
    }
}
