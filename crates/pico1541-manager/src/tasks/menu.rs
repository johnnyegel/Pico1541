use crate::driver::DisplayDriver;
use heapless::Vec;
use heapless::String;
use core::fmt::Write;

pub struct Menu<const N: usize> {
    title: &'static str,
    items: Vec<&'static str, N>,
    selected_index: usize,
    scroll_offset: usize,
}

impl<const N: usize> Menu<N> {
    pub fn new(title: &'static str, items: Vec<&'static str, N>) -> Self {
        Self {
            title,
            items,
            selected_index: 0,
            scroll_offset: 0,
        }
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
        if self.selected_index + 1 < self.items.len() {
            self.selected_index += 1;
            // Assuming 3 lines available for items (Line 1, 2, 3)
            if self.selected_index >= self.scroll_offset + 3 {
                self.scroll_offset = self.selected_index - 2;
            }
        }
    }

    pub fn selected_item(&self) -> &'static str {
        self.items[self.selected_index]
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub async fn render<D: DisplayDriver>(&self, display: &mut D) {
        // Line 0: Title Bar
        display.print(0, self.title).await;

        // Lines 1-3: Menu Items
        for i in 0..3 {
            let item_index = self.scroll_offset + i;
            let mut line = String::<32>::new();
            
            if item_index < self.items.len() {
                if item_index == self.selected_index {
                    let _ = write!(line, "> {}", self.items[item_index]);
                } else {
                    let _ = write!(line, "  {}", self.items[item_index]);
                }
            }
            display.print((i + 1) as u8, &line).await;
        }
        
        display.request_redraw();
    }
}
