use crate::driver::InputDriver;
use crate::KEYBOARD_CHANNEL;
use crate::tasks::button::ButtonManager;
use embassy_time::{Duration, Timer};

pub struct KeyboardTask<I: InputDriver> {
    input: I,
    manager: ButtonManager,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I) -> Self {
        Self {
            input,
            manager: ButtonManager::new(),
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let current_state = self.input.scan();
            self.manager.update(current_state, |event| {
                let _ = KEYBOARD_CHANNEL.try_send(event);
            });
            Timer::after(Duration::from_millis(20)).await;
        }
    }
}
