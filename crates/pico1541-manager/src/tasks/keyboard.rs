use crate::driver::InputDriver;
use crate::{Button, ButtonEvent};
use embassy_time::{Duration, Timer};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;

pub struct KeyboardTask<I: InputDriver> {
    input: I,
    event_tx: Sender<'static, CriticalSectionRawMutex, ButtonEvent, 16>,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I, event_tx: Sender<'static, CriticalSectionRawMutex, ButtonEvent, 16>) -> Self {
        Self { input, event_tx }
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
                                let _ = self.event_tx.send(ButtonEvent::Pressed(button)).await;
                            } else {
                                log::info!("KEYBOARD: Button {:?} RELEASED", button);
                                let _ = self.event_tx.send(ButtonEvent::Released(button)).await;
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
                            let _ = self.event_tx.send(ButtonEvent::Hold(button)).await;
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
