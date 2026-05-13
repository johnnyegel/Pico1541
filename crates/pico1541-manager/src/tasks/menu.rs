use crate::driver::DisplayDriver;
use heapless::Vec;
use core::fmt::Display;

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
            if self.selected_index >= self.scroll_offset + 3 {
                self.scroll_offset = self.selected_index - 2;
            }
        }
    }

    pub async fn render<D: DisplayDriver>(&self, display: &mut D) {
        display.print(0, self.title.as_ref()).await;
        
        for i in 0..3 {
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
}
