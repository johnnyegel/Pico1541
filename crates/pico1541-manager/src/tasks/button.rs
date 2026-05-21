use crate::{Button, ButtonEvent};

pub struct ButtonManager {
    last_state: u8,
    hold_counters: [u16; 4],
}

impl ButtonManager {
    pub const fn new() -> Self {
        Self {
            last_state: 0,
            hold_counters: [0; 4],
        }
    }

    pub fn update(&mut self, current_state: u8, mut callback: impl FnMut(ButtonEvent)) {
        for i in 0..4 {
            let bit = (current_state >> i) & 1;
            let last_bit = (self.last_state >> i) & 1;
            
            let button = match i {
                0 => Button::Back,
                1 => Button::Down,
                2 => Button::Select,
                3 => Button::Up,
                _ => unreachable!(),
            };

            if bit == 1 && last_bit == 0 {
                callback(ButtonEvent::Pressed(button));
                self.hold_counters[i] = 0;
            } else if bit == 0 && last_bit == 1 {
                callback(ButtonEvent::Released(button));
                self.hold_counters[i] = 0;
            } else if bit == 1 && last_bit == 1 {
                self.hold_counters[i] = self.hold_counters[i].saturating_add(1);
                // 300ms at 10ms intervals = 30
                if self.hold_counters[i] == 30 {
                    callback(ButtonEvent::Hold(button));
                } else if self.hold_counters[i] > 30 {
                    let repeat_ticks = 3; // 30ms repeat
                    if (self.hold_counters[i] - 30) % repeat_ticks == 0 {
                        callback(ButtonEvent::Hold(button));
                    }
                }
            }
        }
        self.last_state = current_state;
    }
}
