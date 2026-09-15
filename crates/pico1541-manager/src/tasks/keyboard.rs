use crate::driver::InputDriver;
use crate::{Button, ButtonEvent, KEYBOARD_CHANNEL};
use embassy_time::{with_timeout, Duration, Timer};

pub struct KeyboardTask<I: InputDriver> {
    input: I,
}

impl<I: InputDriver> KeyboardTask<I> {
    pub fn new(input: I) -> Self {
        Self { input }
    }

    pub async fn run(&mut self) -> ! {
        let mut current_state = 0;
        let mut hold_time = Duration::from_millis(300);

        loop {
            let mut timeout_occurred = if current_state == 0 {
                // No buttons pressed, wait indefinitely (avoids Duration::MAX overflow!)
                self.input.wait_for_change().await;
                false
            } else {
                // Button(s) pressed, wait with timeout to trigger Hold events
                let result = with_timeout(hold_time, self.input.wait_for_change()).await;
                result.is_err()
            };

            if timeout_occurred {
                // Before assuming it's a Hold, verify the button is still pressed
                let candidate_state = self.input.scan().await;
                if candidate_state != current_state {
                    // We missed an edge! Let the IRQ logic handle the state change.
                    timeout_occurred = false;
                }
            }

            if timeout_occurred {
                // Timeout occurred! Button(s) are being held.
                for i in 0..4 {
                    if (current_state >> i) & 1 == 1 {
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
                // IRQ fired! A hardware state change or noise spike occurred.
                Timer::after(Duration::from_millis(15)).await; // wait for bounce to settle

                // Read the state 3 times, 5ms apart. All must agree to filter out EMI noise spikes.
                let candidate_state = self.input.scan().await;
                let mut agrees = true;
                for _ in 0..2 {
                    Timer::after(Duration::from_millis(5)).await;
                    if self.input.scan().await != candidate_state {
                        agrees = false;
                        break;
                    }
                }

                if !agrees || candidate_state == current_state {
                    continue; // just bounce noise, ignore
                }

                let new_state = candidate_state;

                let changes = current_state ^ new_state;
                for i in 0..4 {
                    if (changes >> i) & 1 == 1 {
                        let button = match i {
                            0 => Button::Up,
                            1 => Button::Down,
                            2 => Button::Select,
                            3 => Button::Back,
                            _ => unreachable!(),
                        };
                        if (new_state >> i) & 1 == 1 {
                            log::info!("KEYBOARD: Button {:?} PRESSED", button);
                            let _ = KEYBOARD_CHANNEL.try_send(ButtonEvent::Pressed(button));
                        } else {
                            log::info!("KEYBOARD: Button {:?} RELEASED", button);
                            let _ = KEYBOARD_CHANNEL.try_send(ButtonEvent::Released(button));
                        }
                    }
                }

                current_state = new_state;
                if current_state != 0 {
                    hold_time = Duration::from_millis(300); // Start initial hold timer
                }
            }
        }
    }
}
