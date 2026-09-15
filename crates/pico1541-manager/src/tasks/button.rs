use crate::{Button, ButtonEvent};

pub struct ButtonManager {
    last_raw_state: u8,
    debounced_state: u8,
    hold_counters: [u16; 4],
}

impl Default for ButtonManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonManager {
    pub const fn new() -> Self {
        Self {
            last_raw_state: 0,
            debounced_state: 0,
            hold_counters: [0; 4],
        }
    }

    pub fn update(&mut self, current_state: u8, mut callback: impl FnMut(ButtonEvent)) {
        let bounce_mask = current_state ^ self.last_raw_state;
        let stable_mask = !bounce_mask;

        let new_debounced = (self.debounced_state & bounce_mask) | (current_state & stable_mask);
        let changes = self.debounced_state ^ new_debounced;

        self.debounced_state = new_debounced;
        self.last_raw_state = current_state;

        for i in 0..4 {
            let changed = (changes >> i) & 1;
            let bit = (self.debounced_state >> i) & 1;

            let button = match i {
                0 => Button::Up,
                1 => Button::Down,
                2 => Button::Select,
                3 => Button::Back,
                _ => unreachable!(),
            };

            if changed == 1 {
                if bit == 1 {
                    callback(ButtonEvent::Pressed(button));
                    self.hold_counters[i] = 0;
                } else {
                    callback(ButtonEvent::Released(button));
                    self.hold_counters[i] = 0;
                }
            } else if bit == 1 {
                self.hold_counters[i] = self.hold_counters[i].saturating_add(1);
                if self.hold_counters[i] == 30 {
                    callback(ButtonEvent::Hold(button));
                } else if self.hold_counters[i] > 30 {
                    let repeat_ticks = 3; // 30ms repeat
                    if (self.hold_counters[i] - 30).is_multiple_of(repeat_ticks) {
                        callback(ButtonEvent::Hold(button));
                    }
                }
            }
        }
    }
}
