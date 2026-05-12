use crate::driver::{DisplayDriver, InputDriver, RgbLedDriver};
use embassy_time::{Duration, Timer};
use heapless::String;
use core::fmt::Write;

pub struct HardwareTestTask<D, I, L>
where
    D: DisplayDriver,
    I: InputDriver,
    L: RgbLedDriver,
{
    display: D,
    input: I,
    leds: L,
}

impl<D, I, L> HardwareTestTask<D, I, L>
where
    D: DisplayDriver,
    I: InputDriver,
    L: RgbLedDriver,
{
    pub fn new(display: D, input: I, leds: L) -> Self {
        Self {
            display,
            input,
            leds,
        }
    }

    pub async fn run(&mut self) -> ! {
        self.display.clear().await;
        self.display.print(0, "Hardware Test Mode").await;
        
        let mut step = 0;
        loop {
            // 1. Cycle LEDs
            let r = (step % 3) == 0;
            let g = (step % 3) == 1;
            let b = (step % 3) == 2;
            self.leds.set_color(r, g, b);

            // 2. Scan Buttons
            let buttons = self.input.scan();
            
            // 3. Update Display
            let mut status = String::<32>::new();
            let _ = write!(status, "Buttons: {:04b}", buttons);
            self.display.print(1, &status).await;

            let mut led_status = String::<32>::new();
            let color = match step % 3 {
                0 => "RED",
                1 => "GREEN",
                _ => "BLUE",
            };
            let _ = write!(led_status, "LED: {}", color);
            self.display.print(2, &led_status).await;

            self.display.update().await;
            self.display.redraw().await;

            step += 1;
            Timer::after(Duration::from_millis(200)).await;
        }
    }
}
