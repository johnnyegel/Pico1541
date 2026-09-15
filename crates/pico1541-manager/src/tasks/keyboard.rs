use crate::driver::InputDriver;
use crate::{Button, ButtonEvent, KEYBOARD_CHANNEL};
use embassy_time::{Duration, Timer};

pub struct KeyboardTask<I: InputDriver> {
    input: I,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I) -> Self {
        Self { input }
    }

    pub async fn run(&mut self) -> ! {
        let mut stable_state = 0;
        let mut last_sample = 0;
        let mut sample_count = 0;
        let mut hold_time = Duration::from_millis(300);

        loop {
            // Read all button states (acts as reading SIO)
            let current_sample = self.input.scan().await;

            if current_sample == last_sample {
                // State is consistent with the last sample
                if sample_count < 3 {
                    sample_count += 1;
                }
            } else {
                // State changed, reset the counter
                sample_count = 1;
                last_sample = current_sample;
            }

            // If we have 3 consistent samples, consider it stable
            if sample_count == 3 {
                let changes = stable_state ^ current_sample;

                if changes != 0 {
                    for i in 0..4 {
                        if (changes >> i) & 1 == 1 {
                            let button = match i {
                                0 => Button::Up,
                                1 => Button::Down,
                                2 => Button::Select,
                                3 => Button::Back,
                                _ => unreachable!(),
                            };

                            if (current_sample >> i) & 1 == 1 {
                                log::info!("KEYBOARD: Button {:?} PRESSED", button);
                                let _ = KEYBOARD_CHANNEL.try_send(ButtonEvent::Pressed(button));
                            } else {
                                log::info!("KEYBOARD: Button {:?} RELEASED", button);
                                let _ = KEYBOARD_CHANNEL.try_send(ButtonEvent::Released(button));
                            }
                        }
                    }
                    stable_state = current_sample;

                    if stable_state != 0 {
                        hold_time = Duration::from_millis(300); // Start initial hold timer
                    }
                }
            }

            // Handle Hold events if a button is stable and pressed
            if stable_state != 0 {
                if hold_time.as_millis() == 0 {
                    for i in 0..4 {
                        if (stable_state >> i) & 1 == 1 {
                            let button = match i {
                                0 => Button::Up,
                                1 => Button::Down,
                                2 => Button::Select,
                                3 => Button::Back,
                                _ => unreachable!(),
                            };
                            log::info!("KEYBOARD: Button {:?} HELD", button);
                            let _ = KEYBOARD_CHANNEL.try_send(ButtonEvent::Hold(button));
                        }
                    }
                    hold_time = Duration::from_millis(30); // Repeat hold timer
                } else {
                    hold_time = Duration::from_millis(hold_time.as_millis().saturating_sub(10));
                }
            }

            Timer::after(Duration::from_millis(10)).await;
        }
    }
}
