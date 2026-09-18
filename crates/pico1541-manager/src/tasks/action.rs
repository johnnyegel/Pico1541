use crate::{Button, ButtonEvent, UiAction};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Receiver, Sender};

pub struct ActionTask {
    back_held: bool,
    select_held: bool,
    back_used_for_chord: bool,
    action_fired_on_hold: bool,
    scroll_hold_count: usize,
    event_rx: Receiver<'static, CriticalSectionRawMutex, ButtonEvent, 16>,
    action_tx: Sender<'static, CriticalSectionRawMutex, UiAction, 16>,
}

impl ActionTask {
    pub fn new(
        event_rx: Receiver<'static, CriticalSectionRawMutex, ButtonEvent, 16>,
        action_tx: Sender<'static, CriticalSectionRawMutex, UiAction, 16>,
    ) -> Self {
        Self {
            back_held: false,
            select_held: false,
            back_used_for_chord: false,
            action_fired_on_hold: false,
            scroll_hold_count: 0,
            event_rx,
            action_tx,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            let event = self.event_rx.receive().await;

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
                        let _ = self.action_tx.send(UiAction::SkipUp).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::ScrollUp).await;
                    }
                }
                ButtonEvent::Hold(Button::Up) => {
                    self.scroll_hold_count = self.scroll_hold_count.saturating_add(1);
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = self.action_tx.send(UiAction::SkipUp).await;
                    } else if self.scroll_hold_count > 15 {
                        let _ = self.action_tx.send(UiAction::FastScrollUp).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::ScrollUp).await;
                    }
                }
                ButtonEvent::Pressed(Button::Down) => {
                    self.scroll_hold_count = 0;
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = self.action_tx.send(UiAction::SkipDown).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::ScrollDown).await;
                    }
                }
                ButtonEvent::Hold(Button::Down) => {
                    self.scroll_hold_count = self.scroll_hold_count.saturating_add(1);
                    if self.back_held {
                        self.back_used_for_chord = true;
                        let _ = self.action_tx.send(UiAction::SkipDown).await;
                    } else if self.scroll_hold_count > 15 {
                        let _ = self.action_tx.send(UiAction::FastScrollDown).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::ScrollDown).await;
                    }
                }
                ButtonEvent::Hold(Button::Back) => {
                    if self.select_held {
                        let _ = self.action_tx.send(UiAction::Monitor).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::Home).await;
                    }
                    self.action_fired_on_hold = true;
                }
                ButtonEvent::Hold(Button::Select) => {
                    if self.back_held {
                        let _ = self.action_tx.send(UiAction::Monitor).await;
                    } else {
                        let _ = self.action_tx.send(UiAction::ContextAction).await;
                    }
                    self.action_fired_on_hold = true;
                }
                ButtonEvent::Released(Button::Back) => {
                    if self.back_held && !self.back_used_for_chord && !self.select_held {
                        let _ = self.action_tx.send(UiAction::Back).await;
                    }
                    self.back_held = false;
                    self.back_used_for_chord = false;
                }
                ButtonEvent::Released(Button::Select) => {
                    if self.select_held && !self.back_held {
                        let _ = self.action_tx.send(UiAction::Select).await;
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
