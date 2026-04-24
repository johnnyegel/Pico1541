use embassy_time::{Duration, Timer};
use crate::driver::DisplayDriver;

pub struct DisplayTask<D: DisplayDriver> {
    display: D,
}

impl<D: DisplayDriver> DisplayTask<D> {
    pub fn new(display: D) -> Self {
        Self { display }
    }

    pub async fn run(&mut self) -> ! {
        self.display.clear().await;
        self.display.print(0, "Pico1541 Ready").await;

        loop {
            self.display.update().await;
            self.display.redraw().await;
            Timer::after(Duration::from_millis(50)).await;
        }
    }
}
