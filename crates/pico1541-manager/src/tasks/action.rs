use crate::{Button, ButtonEvent, UiAction, ACTION_CHANNEL, KEYBOARD_CHANNEL};

pub struct ActionTask {
    back_held: bool,
    select_held: bool,
    back_used_for_chord: bool,
    action_fired_on_hold: bool,
    scroll_hold_count: usize,
}

impl Default for ActionTask {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionTask {
    pub const fn new() -> Self {
        Self {
            back_held: false,
            select_held: false,
            back_used_for_chord: false,
            action_fired_on_hold: false,
            scroll_hold_count: 0,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = KEYBOARD_CHANNEL.receive().await;

            if self.action_fired_on_hold {
                match event {
                    ButtonEvent::Released(Button::Back) => {
                        self.back_held = false;
                        if !self.back_held && !self.select_held {
                            self.action_fired_on_hold = false;
                        }
                    }
                    ButtonEvent::Released(Button::Select) => {
                        self.select_held = false;
                        if !self.back_held && !self.select_held {
                            self.action_fired_on_hold = false;
                        }
                    }
                    ButtonEvent::Released(Button::Up) | ButtonEvent::Released(Button::Down) => {
                        self.scroll_hold_count = 0;
                    }
                    _ => {}
                }
                continue;
            }

            match event {
                ButtonEvent::Pressed(Button::Back) => {
                    self.back_held = true;
                    self.back_used_for_chord = false;
                }
                ButtonEvent::Pressed(Button::Select) => {
                    self.select_held = true;
                }
                ButtonEvent::Pressed(Button::Up) => {
                    self.scroll_hold_count = 0;
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipUp);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollUp);
                    }
                }
                ButtonEvent::Hold(Button::Up) => {
                    self.scroll_hold_count = self.scroll_hold_count.saturating_add(1);
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipUp);
                    } else if self.scroll_hold_count > 15 {
                        let _ = ACTION_CHANNEL.try_send(UiAction::FastScrollUp);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollUp);
                    }
                }
                ButtonEvent::Pressed(Button::Down) => {
                    self.scroll_hold_count = 0;
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipDown);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollDown);
                    }
                }
                ButtonEvent::Hold(Button::Down) => {
                    self.scroll_hold_count = self.scroll_hold_count.saturating_add(1);
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = ACTION_CHANNEL.try_send(UiAction::SkipDown);
                    } else if self.scroll_hold_count > 15 {
                        let _ = ACTION_CHANNEL.try_send(UiAction::FastScrollDown);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ScrollDown);
                    }
                }
                ButtonEvent::Hold(Button::Back) => {
                    if self.select_held {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Monitor);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Home);
                    }
                    self.action_fired_on_hold = true;
                }
                ButtonEvent::Hold(Button::Select) => {
                    if self.back_held {
                        let _ = ACTION_CHANNEL.try_send(UiAction::Monitor);
                    } else {
                        let _ = ACTION_CHANNEL.try_send(UiAction::ContextAction);
                    }
                    self.action_fired_on_hold = true;
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
                ButtonEvent::Released(Button::Up) | ButtonEvent::Released(Button::Down) => {
                    self.scroll_hold_count = 0;
                }
            }
        }
    }
}
