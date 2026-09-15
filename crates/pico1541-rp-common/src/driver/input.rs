use embassy_rp::gpio::{Input, Level, OutputOpenDrain, Pin, Pull};
use embassy_rp::Peri;
use pico1541_manager::InputDriver;

/// Driver for the 2x2 Keyboard Matrix (Rows: 6, 7; Cols: 8, 9).
pub struct KeyboardInput {
    rows: [OutputOpenDrain<'static>; 2],
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
                OutputOpenDrain::new(row0, Level::High),
                OutputOpenDrain::new(row1, Level::High),
            ],
            cols: [Input::new(col0, Pull::Up), Input::new(col1, Pull::Up)],
        }
    }
}

impl InputDriver for KeyboardInput {
    async fn wait_for_change(&mut self) {
        // Assert all rows LOW so any button press pulls a column LOW
        for row in self.rows.iter_mut() {
            row.set_low();
        }

        // Delay to let the signals settle (RC time constant of pullups + capacitance)
        embassy_time::Timer::after_micros(1000).await;

        // Wait for a falling or rising edge on any column.
        let (col0_slice, col1_slice) = self.cols.split_at_mut(1);
        let _ = embassy_futures::select::select(
            col0_slice[0].wait_for_any_edge(),
            col1_slice[0].wait_for_any_edge(),
        )
        .await;
    }

    async fn scan(&mut self) -> u8 {
        let mut pressed = 0;

        // Ensure all rows are HIGH (Open Drain = floating) before scanning to avoid ghosting
        for row in self.rows.iter_mut() {
            row.set_high();
        }
        // Let the column pull-ups pull the lines HIGH.
        embassy_time::Timer::after_micros(1000).await;

        for (r, row) in self.rows.iter_mut().enumerate() {
            row.set_low();
            // Delay for signal to settle (discharge capacitance through the switch)
            embassy_time::Timer::after_micros(1000).await;
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
