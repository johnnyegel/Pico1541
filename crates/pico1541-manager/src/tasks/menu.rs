use crate::driver::DisplayDriver;
use crate::{KEYBOARD_CHANNEL, Button, ButtonEvent};
use heapless::Vec;
use core::fmt::Display;

pub enum MenuEvent {
    Selected(usize),
    Back,
}

pub struct Menu<T, const N: usize, Title = &'static str> {
    title: Title,
    items: Vec<T, N>,
    selected_index: usize,
    scroll_offset: usize,
}

impl<T: Display + AsRef<str>, const N: usize, Title: AsRef<str>> Menu<T, N, Title> {
    pub fn new(title: Title, items: Vec<T, N>) -> Self {
        Self {
            title,
            items,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_cursor(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = index;
            // Adjust scroll offset if needed
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            } else if self.selected_index >= self.scroll_offset + 5 {
                self.scroll_offset = self.selected_index - 4;
            }
        }
    }

    pub fn selected_item(&self) -> &T {
        &self.items[self.selected_index]
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.items.len() - 1 {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + 5 {
                self.scroll_offset = self.selected_index - 4;
            }
        }
    }

    pub async fn render<D: DisplayDriver>(&self, display: &mut D) {
        display.print(0, self.title.as_ref()).await;
        
        for i in 0..5 {
            let item_idx = self.scroll_offset + i;
            if item_idx < self.items.len() {
                let prefix = if item_idx == self.selected_index { ">" } else { " " };
                let mut line_buf = [0u8; 16];
                let item_str = self.items[item_idx].as_ref();
                let len = item_str.len().min(15);
                line_buf[0] = prefix.as_bytes()[0];
                line_buf[1..1+len].copy_from_slice(&item_str.as_bytes()[..len]);
                
                if let Ok(s) = core::str::from_utf8(&line_buf[..1+len]) {
                    display.print((i + 1) as u8, s).await;
                }
            }
        }
        display.request_redraw();
    }

    pub async fn run<D: DisplayDriver>(&mut self, display: &mut D) -> MenuEvent {
        loop {
            display.clear().await;
            self.render(display).await;

            match KEYBOARD_CHANNEL.receive().await {
                ButtonEvent::Pressed(Button::Up) | ButtonEvent::Hold(Button::Up) => {
                    self.move_up();
                }
                ButtonEvent::Pressed(Button::Down) | ButtonEvent::Hold(Button::Down) => {
                    self.move_down();
                }
                ButtonEvent::Pressed(Button::Select) => {
                    return MenuEvent::Selected(self.selected_index);
                }
                ButtonEvent::Pressed(Button::Back) => {
                    return MenuEvent::Back;
                }
                _ => {}
            }
        }
    }
}
