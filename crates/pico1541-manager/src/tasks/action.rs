use crate::{Button, ButtonEvent, UiAction, KEYBOARD_CHANNEL, ACTION_CHANNEL};

pub struct ActionTask {
    back_held: bool,
    select_held: bool,
    back_used_for_chord: bool,
    back_hold_count: usize,
    select_hold_count: usize,
}

impl ActionTask {
    pub const fn new() -> Self {
        Self {
            back_held: false,
            select_held: false,
            back_used_for_chord: false,
            back_hold_count: 0,
            select_hold_count: 0,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = KEYBOARD_CHANNEL.receive().await;

            match event {
                ButtonEvent::Pressed(Button::Back) => {
                    self.back_hold_count = 0;
                    self.back_held = true;
                    self.back_used_for_chord = false;
                }
                ButtonEvent::Pressed(Button::Select) => {
                    self.select_hold_count = 0;
                    self.select_held = true;
                }
                ButtonEvent::Pressed(Button::Up) | ButtonEvent::Hold(Button::Up) => {
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipUp);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollUp);
                    }
                }
                ButtonEvent::Pressed(Button::Down) | ButtonEvent::Hold(Button::Down) => {
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipDown);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollDown);
                    }
                }
                ButtonEvent::Hold(Button::Back) => {
                    self.back_hold_count += 1;
                    if self.back_hold_count >= 24 {
                        if self.select_held {
                            let _ = ACTION_CHANNEL.try_send(UiAction::Home);
                            self.back_held = false;
                            self.select_held = false;
                        }
                        // Hold Back alone does nothing (just waits for chord)
                    }
                }
                ButtonEvent::Hold(Button::Select) => {
                    self.select_hold_count += 1;
                    if self.select_hold_count >= 24 && self.back_held {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Home);
                        self.back_held = false;
                        self.select_held = false;
                    }
                }
                ButtonEvent::Released(Button::Back) => {
                    if self.back_held && !self.back_used_for_chord && !self.select_held {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Back);
                    }
                    self.back_held = false;
                    self.back_used_for_chord = false;
                }
                ButtonEvent::Released(Button::Select) => {
                    if self.select_held && !self.back_held {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Select);
                    }
                    self.select_held = false;
                }
                _ => {}
            }
        }
    }
}
