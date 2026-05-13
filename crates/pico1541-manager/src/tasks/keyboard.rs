use crate::driver::InputDriver;
use crate::KEYBOARD_CHANNEL;
use embassy_time::{Duration, Timer};

pub struct KeyboardTask<I: InputDriver> {
    input: I,
    last_state: u8,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I) -> Self {
        Self {
            input,
            last_state: 0,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let current_state = self.input.scan();
            if current_state != self.last_state {
                let _ = KEYBOARD_CHANNEL.try_send(current_state);
                self.last_state = current_state;
            }
            Timer::after(Duration::from_millis(20)).await;
        }
    }
}
