use crate::driver::InputDriver;

pub struct KeyboardTask<I: InputDriver> {
    input: I,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I) -> Self {
        Self { input }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let _btn = self.input.wait_for_button().await;
            // Handle button/key events
        }
    }
}
