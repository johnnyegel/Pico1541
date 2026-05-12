use embassy_rp::gpio::{Input, Level, Output, Pin, Pull};
use embassy_rp::Peri;
use embassy_time::{Duration, Timer};
use pico1541_manager::InputDriver;

/// Driver for the 2x2 Keyboard Matrix (Rows: 6, 7; Cols: 8, 9).
pub struct KeyboardInput {
    rows: [Output<'static>; 2],
    cols: [Input<'static>; 2],
}

impl KeyboardInput {
    pub fn new(
        row0: Peri<'static, impl Pin>,
        row1: Peri<'static, impl Pin>,
        col0: Peri<'static, impl Pin>,
        col1: Peri<'static, impl Pin>,
    ) -> Self {
        Self {
            rows: [
                Output::new(row0, Level::High),
                Output::new(row1, Level::High),
            ],
            cols: [
                Input::new(col0, Pull::Up),
                Input::new(col1, Pull::Up),
            ],
        }
    }
}

impl InputDriver for KeyboardInput {
    async fn wait_for_button(&mut self) -> u8 {
        loop {
            let pressed = self.scan();
            if pressed != 0 {
                return pressed;
            }
            Timer::after(Duration::from_millis(20)).await;
        }
    }

    fn scan(&mut self) -> u8 {
        let mut pressed = 0;
        for (r, row) in self.rows.iter_mut().enumerate() {
            row.set_low();
            // Small delay for signal to settle
            cortex_m::asm::delay(100);
            for (c, col) in self.cols.iter().enumerate() {
                if col.is_low() {
                    pressed |= 1 << (r * 2 + c);
                }
            }
            row.set_high();
        }
        pressed
    }
}
