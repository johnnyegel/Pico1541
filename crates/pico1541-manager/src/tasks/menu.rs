use crate::driver::DisplayDriver;
use crate::{KEYBOARD_CHANNEL, Button, ButtonEvent};
use heapless::Vec;
use core::fmt::Display;

pub enum MenuEvent {
    Selected(usize),
    Back,
    Home,
    LoadBlock(usize),
}

pub struct Menu<T, const N: usize, Title = &'static str> {
    title: Title,
    items: [Option<T>; N],
    start_block: usize,
    end_block: usize,
    total_count: usize,
    selected_index: usize,
    scroll_offset: usize,
}

impl<T: Clone + Display + AsRef<str>, const N: usize, Title: AsRef<str>> Menu<T, N, Title> {
    pub fn new(title: Title, items: Vec<T, N>) -> Self {
        let mut buffer = core::array::from_fn(|_| None);
        
        let total_count = items.len();
        for (i, item) in items.iter().enumerate() {
            if i < N {
                buffer[i] = Some(item.clone());
            }
        }
        
        let total_blocks = (total_count + 15) / 16;
        Self {
            title,
            items: buffer,
            start_block: 0,
            end_block: total_blocks,
            total_count,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    pub fn new_paged(title: Title, total_count: usize) -> Self {
        Self {
            title,
            items: core::array::from_fn(|_| None),
            start_block: 0,
            end_block: 0,
            total_count,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    pub fn set_block(&mut self, block_index: usize, block_items: &[T]) {
        let slot = block_index % 3;
        let start_slot_idx = slot * 16;
        for i in 0..16 {
            if i < block_items.len() {
                self.items[start_slot_idx + i] = Some(block_items[i].clone());
            } else {
                self.items[start_slot_idx + i] = None;
            }
        }
        
        if self.start_block == self.end_block {
            self.start_block = block_index;
            self.end_block = block_index + 1;
        } else if block_index == self.end_block {
            self.end_block += 1;
            if self.end_block - self.start_block > 3 {
                self.start_block = self.end_block - 3;
            }
        } else if block_index + 1 == self.start_block {
            self.start_block = block_index;
            if self.end_block - self.start_block > 3 {
                self.end_block = self.start_block + 3;
            }
        } else if block_index >= self.start_block && block_index < self.end_block {
            // Already in range, do nothing
        } else {
            self.start_block = block_index;
            self.end_block = block_index + 1;
        }
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn get_item(&self, idx: usize) -> Option<&T> {
        if self.total_count <= N {
            self.items.get(idx).and_then(|opt| opt.as_ref())
        } else {
            let b = idx / 16;
            if b >= self.start_block && b < self.end_block {
                let slot = b % 3;
                self.items.get(slot * 16 + (idx % 16)).and_then(|opt| opt.as_ref())
            } else {
                None
            }
        }
    }

    pub fn set_cursor(&mut self, index: usize) -> Option<usize> {
        if index < self.total_count {
            self.selected_index = index;
            // Adjust scroll offset if needed
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            } else if self.selected_index >= self.scroll_offset + 5 {
                self.scroll_offset = self.selected_index - 4;
            }
            
            if self.total_count > N {
                let current_block = self.selected_index / 16;
                let total_blocks = (self.total_count + 15) / 16;
                let targets = target_blocks(current_block, total_blocks);
                for tb in targets.into_iter().flatten() {
                    if tb < self.start_block || tb >= self.end_block {
                        return Some(tb);
                    }
                }
            }
        }
        None
    }

    pub fn selected_item(&self) -> Option<&T> {
        self.get_item(self.selected_index)
    }

    pub fn move_up(&mut self) -> Option<usize> {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
            if self.total_count > N {
                let current_block = self.selected_index / 16;
                let total_blocks = (self.total_count + 15) / 16;
                let targets = target_blocks(current_block, total_blocks);
                for tb in targets.into_iter().flatten() {
                    if tb < self.start_block || tb >= self.end_block {
                        return Some(tb);
                    }
                }
            }
        }
        None
    }

    pub fn move_down(&mut self) -> Option<usize> {
        if self.selected_index < self.total_count - 1 {
            self.selected_index += 1;
            if self.selected_index >= self.scroll_offset + 5 {
                self.scroll_offset = self.selected_index - 4;
            }
            if self.total_count > N {
                let current_block = self.selected_index / 16;
                let total_blocks = (self.total_count + 15) / 16;
                let targets = target_blocks(current_block, total_blocks);
                for tb in targets.into_iter().flatten() {
                    if tb < self.start_block || tb >= self.end_block {
                        return Some(tb);
                    }
                }
            }
        }
        None
    }

    pub async fn render<D: DisplayDriver>(&self, display: &mut D) {
        display.print(0, self.title.as_ref()).await;
        
        let screen_selected_line = (self.selected_index - self.scroll_offset + 1) as u8;
        display.set_selected_line(screen_selected_line);
        
        for i in 0..5 {
            let item_idx = self.scroll_offset + i;
            if item_idx < self.total_count {
                let prefix = if item_idx == self.selected_index { "> " } else { "  " };
                let mut line_buf = [0u8; 64];
                if let Some(item) = self.get_item(item_idx) {
                    let item_str = item.as_ref();
                    let len = item_str.len().min(62);
                    line_buf[0..2].copy_from_slice(prefix.as_bytes());
                    line_buf[2..2+len].copy_from_slice(&item_str.as_bytes()[..len]);
                    
                    if let Ok(s) = core::str::from_utf8(&line_buf[..2+len]) {
                        display.print((i + 1) as u8, s).await;
                    }
                } else {
                    display.print((i + 1) as u8, "...Loading...").await;
                }
            }
        }
        display.request_redraw();
    }

    pub async fn run<D: DisplayDriver>(&mut self, display: &mut D) -> MenuEvent {
        let mut back_hold_count = 0;
        loop {
            display.clear().await;
            self.render(display).await;

            match KEYBOARD_CHANNEL.receive().await {
                ButtonEvent::Pressed(Button::Up) | ButtonEvent::Hold(Button::Up) => {
                    if let Some(block_to_load) = self.move_up() {
                        return MenuEvent::LoadBlock(block_to_load);
                    }
                }
                ButtonEvent::Pressed(Button::Down) | ButtonEvent::Hold(Button::Down) => {
                    if let Some(block_to_load) = self.move_down() {
                        return MenuEvent::LoadBlock(block_to_load);
                    }
                }
                ButtonEvent::Pressed(Button::Select) => {
                    return MenuEvent::Selected(self.selected_index);
                }
                ButtonEvent::Pressed(Button::Back) => {
                    back_hold_count = 0;
                }
                ButtonEvent::Hold(Button::Back) => {
                    back_hold_count += 1;
                    if back_hold_count >= 24 {
                        return MenuEvent::Home;
                    }
                }
                ButtonEvent::Released(Button::Back) => {
                    if back_hold_count < 24 {
                        return MenuEvent::Back;
                    }
                }
                _ => {}
            }
        }
    }
}

fn target_blocks(current_block: usize, total_blocks: usize) -> [Option<usize>; 3] {
    if total_blocks <= 3 {
        let mut targets = [None; 3];
        for i in 0..total_blocks {
            targets[i] = Some(i);
        }
        targets
    } else if current_block == 0 {
        [Some(0), Some(1), Some(2)]
    } else if current_block + 1 >= total_blocks {
        [Some(total_blocks - 3), Some(total_blocks - 2), Some(total_blocks - 1)]
    } else {
        [Some(current_block - 1), Some(current_block), Some(current_block + 1)]
    }
}
