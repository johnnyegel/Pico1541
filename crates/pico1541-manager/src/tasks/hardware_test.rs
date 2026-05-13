use crate::driver::{DisplayDriver, InputDriver, RgbLedDriver, IndicatorDriver, ActivityColor};
use embassy_time::{Duration, Timer};
use heapless::String;
use core::fmt::Write;

pub struct HardwareTestTask<D, I, L, Ind>
where
    D: DisplayDriver,
    I: InputDriver,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
{
    display: D,
    input: I,
    leds: L,
    indicator: Ind,
}

impl<D, I, L, Ind> HardwareTestTask<D, I, L, Ind>
where
    D: DisplayDriver,
    I: InputDriver,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
{
    pub fn new(display: D, input: I, leds: L, indicator: Ind) -> Self {
        Self {
            display,
            input,
            leds,
            indicator,
        }
    }

    pub async fn run(&mut self) -> ! {
        self.display.clear().await;
        self.display.print(0, "Hardware Test Mode").await;
        
        let mut step = 0;
        loop {
            // 1. Cycle Status RGB LEDs (Red -> Green -> Blue -> Off)
            let (r, g, b) = match step % 4 {
                0 => (true, false, false),
                1 => (false, true, false),
                2 => (false, false, true),
                _ => (false, false, false),
            };
            self.leds.set_color(r, g, b);

            // 2. Cycle Activity LED (Red -> Green -> Off)
            let act_color = match step % 3 {
                0 => ActivityColor::Red,
                1 => ActivityColor::Green,
                _ => ActivityColor::Off,
            };
            self.indicator.set_activity_led(act_color);

            // 3. Scan Buttons
            let buttons = self.input.scan();
            
            // 4. Update Display
            let mut status = String::<32>::new();
            let _ = write!(status, "Buttons: {:04b}", buttons);
            self.display.print(1, &status).await;

            let s_color = match step % 4 {
                0 => "RED",
                1 => "GREEN",
                2 => "BLUE",
                _ => "OFF",
            };
            let mut st_line = String::<32>::new();
            let _ = write!(st_line, "Status: {}", s_color);
            self.display.print(2, &st_line).await;

            let a_color = match step % 3 {
                0 => "RED",
                1 => "GREEN",
                _ => "OFF",
            };
            let mut act_line = String::<32>::new();
            let _ = write!(act_line, "Activity: {}", a_color);
            self.display.print(3, &act_line).await;

            self.display.update().await;
            self.display.redraw().await;

            step += 1;
            Timer::after(Duration::from_millis(500)).await;
        }
    }
}
