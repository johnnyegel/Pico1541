use crate::driver::DisplayDriver;
use crate::{ACTION_CHANNEL, UiAction};
use heapless::Vec;
use core::fmt::Display;

pub enum MenuEvent {
    Selected(usize),
    Back,
    Home,
    Monitor,
    ContextAction(usize),
    LoadBlock(usize),
    Timeout,
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
        
        let total_blocks = total_count.div_ceil(16);
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

    /// Create a paged menu whose total item count is not known upfront.
    /// The total will be discovered lazily when a partial block is received via `set_block`.
    pub fn new_paged(title: Title) -> Self {
        Self {
            title,
            items: core::array::from_fn(|_| None),
            start_block: 0,
            end_block: 0,
            total_count: usize::MAX,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    pub fn set_block(&mut self, block_index: usize, block_items: &[T]) {
        let slot = block_index % (N / 16);
        let start_slot_idx = slot * 16;
        for i in 0..16 {
            if i < block_items.len() {
                self.items[start_slot_idx + i] = Some(block_items[i].clone());
            } else {
                self.items[start_slot_idx + i] = None;
            }
        }

        if block_items.len() < 16 {
            let discovered_total = block_index * 16 + block_items.len();
            if discovered_total < self.total_count {
                self.total_count = discovered_total;
                
                // Clamp selected_index if we discovered the end is earlier than we thought
                if self.total_count > 0 && self.selected_index >= self.total_count {
                    self.selected_index = self.total_count - 1;
                    if self.selected_index < self.scroll_offset {
                        self.scroll_offset = self.selected_index;
                    } else if self.selected_index >= self.scroll_offset + 5 {
                        self.scroll_offset = self.selected_index.saturating_sub(4);
                    }
                } else if self.total_count == 0 {
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                }
            }
        }
        
        if self.start_block == self.end_block {
            self.start_block = block_index;
            self.end_block = block_index + 1;
        } else if block_index == self.end_block {
            self.end_block += 1;
            if self.end_block - self.start_block > (N / 16) {
                self.start_block = self.end_block - (N / 16);
            }
        } else if block_index + 1 == self.start_block {
            self.start_block = block_index;
            if self.end_block - self.start_block > (N / 16) {
                self.end_block = self.start_block + (N / 16);
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

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset;
    }

    pub fn get_item(&self, idx: usize) -> Option<&T> {
        if self.total_count <= N {
            self.items.get(idx).and_then(|opt| opt.as_ref())
        } else {
            let b = idx / 16;
            if b >= self.start_block && b < self.end_block {
                let slot = b % (N / 16);
                self.items.get(slot * 16 + (idx % 16)).and_then(|opt| opt.as_ref())
            } else {
                None
            }
        }
    }

    pub fn set_cursor(&mut self, index: usize) -> Option<usize> {
        // Guard against exceeding known total (which may be MAX before end is discovered).
        if self.total_count != usize::MAX && index >= self.total_count {
            return None;
        }
        self.selected_index = index;
        // Adjust scroll offset if needed
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + 5 {
            self.scroll_offset = self.selected_index - 4;
        }
        
        if self.total_count > N {
            let current_block = self.selected_index / 16;
            // Use end_block as a proxy for known total_blocks when total is not yet known.
            let known_blocks = if self.total_count == usize::MAX {
                self.end_block.max(current_block + 1)
            } else {
                self.total_count.div_ceil(16)
            };
            let targets = target_blocks(current_block, known_blocks);
            for tb in targets.into_iter().flatten() {
                if tb < self.start_block || tb >= self.end_block {
                    return Some(tb);
                }
            }
        }
        None
    }

    pub fn prefetch_target(&self) -> Option<usize> {
        if self.total_count > N {
            let current_block = self.selected_index / 16;
            let known_blocks = if self.total_count == usize::MAX {
                self.end_block.max(current_block + 2)
            } else {
                self.total_count.div_ceil(16)
            };
            
            for offset in 1..=2 {
                let tb = current_block + offset;
                if tb < known_blocks && (tb < self.start_block || tb >= self.end_block) {
                    return Some(tb);
                }
            }
            for offset in 1..=2 {
                let tb2 = current_block.saturating_sub(offset);
                if tb2 < self.start_block || tb2 >= self.end_block {
                    return Some(tb2);
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
                let known_blocks = if self.total_count == usize::MAX {
                    self.end_block.max(current_block + 1)
                } else {
                    self.total_count.div_ceil(16)
                };
                let targets = target_blocks(current_block, known_blocks);
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
        // When total is unknown we allow moving down as long as the next slot is either
        // loaded or we haven't yet discovered the end of directory.
        let next = self.selected_index + 1;
        let at_known_end = self.total_count != usize::MAX && next >= self.total_count;
        if !at_known_end {
            self.selected_index = next;
            if self.selected_index >= self.scroll_offset + 5 {
                self.scroll_offset = self.selected_index - 4;
            }
            if self.total_count > N {
                let current_block = self.selected_index / 16;
                let known_blocks = if self.total_count == usize::MAX {
                    self.end_block.max(current_block + 1)
                } else {
                    self.total_count.div_ceil(16)
                };
                let targets = target_blocks(current_block, known_blocks);
                for tb in targets.into_iter().flatten() {
                    if tb < self.start_block || tb >= self.end_block {
                        return Some(tb);
                    }
                }
            }
        }
        None
    }

    pub fn skip_up(&mut self, amount: usize) -> Option<usize> {
        let diff = self.selected_index.saturating_sub(self.scroll_offset);
        self.selected_index = self.selected_index.saturating_sub(amount);
        self.scroll_offset = self.selected_index.saturating_sub(diff);

        if self.total_count > N {
            let current_block = self.selected_index / 16;
            let known_blocks = if self.total_count == usize::MAX {
                self.end_block.max(current_block + 1)
            } else {
                self.total_count.div_ceil(16)
            };
            let targets = target_blocks(current_block, known_blocks);
            for tb in targets.into_iter().flatten() {
                if tb < self.start_block || tb >= self.end_block {
                    return Some(tb);
                }
            }
        }
        None
    }

    pub fn skip_down(&mut self, amount: usize) -> Option<usize> {
        let diff = self.selected_index.saturating_sub(self.scroll_offset);
        let next = self.selected_index.saturating_add(amount);
        let next = if self.total_count != usize::MAX && next >= self.total_count {
            if self.total_count > 0 {
                self.total_count - 1
            } else {
                0
            }
        } else {
            next
        };
        
        self.selected_index = next;
        
        let mut new_scroll = self.selected_index.saturating_sub(diff);
        if self.selected_index < new_scroll {
            new_scroll = self.selected_index;
        } else if self.selected_index >= new_scroll + 5 {
            new_scroll = self.selected_index.saturating_sub(4);
        }
        self.scroll_offset = new_scroll;

        if self.total_count > N {
            let current_block = self.selected_index / 16;
            let known_blocks = if self.total_count == usize::MAX {
                self.end_block.max(current_block + 1)
            } else {
                self.total_count.div_ceil(16)
            };
            let targets = target_blocks(current_block, known_blocks);
            for tb in targets.into_iter().flatten() {
                if tb < self.start_block || tb >= self.end_block {
                    return Some(tb);
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
            // Stop rendering past the known total; if total is unknown render up to loaded range.
            if self.total_count != usize::MAX && item_idx >= self.total_count {
                break;
            }
            let mut line_buf = [0u8; 64];
            if let Some(item) = self.get_item(item_idx) {
                let item_str_full = item.as_ref();
                let is_mounted = item_str_full.starts_with('*');
                let item_str = if is_mounted { &item_str_full[1..] } else { item_str_full };

                let mut prefix_buf = [b' ', b' '];
                if item_idx == self.selected_index {
                    prefix_buf[0] = b'>';
                }
                if is_mounted {
                    prefix_buf[1] = b'*';
                }

                let len = item_str.len().min(62);
                line_buf[0..2].copy_from_slice(&prefix_buf);
                line_buf[2..2+len].copy_from_slice(&item_str.as_bytes()[..len]);
                
                if let Ok(s) = core::str::from_utf8(&line_buf[..2+len]) {
                    display.print((i + 1) as u8, s).await;
                }
            } else {
                // Item not yet loaded (block being prefetched) — show placeholder only
                // if we haven't confirmed this is past the end.
                if self.total_count == usize::MAX || item_idx < self.total_count {
                    display.print((i + 1) as u8, "...Loading...").await;
                }
            }
        }
        display.request_redraw();
    }

    pub async fn run<D: DisplayDriver>(&mut self, display: &mut D) -> MenuEvent {
        loop {
            for i in 0..5 {
                let item_idx = self.scroll_offset + i;
                if self.total_count != usize::MAX && item_idx >= self.total_count {
                    break;
                }
                if self.get_item(item_idx).is_none() {
                    let block_to_load = item_idx / 16;
                    return MenuEvent::LoadBlock(block_to_load);
                }
            }

            display.clear().await;
            self.render(display).await;

            match embassy_time::with_timeout(embassy_time::Duration::from_millis(500), ACTION_CHANNEL.receive()).await {
                Ok(action) => match action {
                    UiAction::ScrollUp => {
                        if let Some(block_to_load) = self.move_up() {
                            return MenuEvent::LoadBlock(block_to_load);
                        }
                    }
                    UiAction::ScrollDown => {
                        if let Some(block_to_load) = self.move_down() {
                            return MenuEvent::LoadBlock(block_to_load);
                        }
                    }
                    UiAction::SkipUp => {
                        if let Some(block_to_load) = self.skip_up(48) {
                            return MenuEvent::LoadBlock(block_to_load);
                        }
                    }
                    UiAction::SkipDown => {
                        if let Some(block_to_load) = self.skip_down(48) {
                            return MenuEvent::LoadBlock(block_to_load);
                        }
                    }
                    UiAction::Select => {
                        return MenuEvent::Selected(self.selected_index);
                    }
                    UiAction::Back => {
                        return MenuEvent::Back;
                    }
                    UiAction::Home => {
                        return MenuEvent::Home;
                    }
                    UiAction::Monitor => {
                        return MenuEvent::Monitor;
                    }
                    UiAction::ContextAction => {
                        return MenuEvent::ContextAction(self.selected_index);
                    }
                },
                Err(_) => {
                    return MenuEvent::Timeout;
                }
            }
        }
    }
}

fn target_blocks(current_block: usize, total_blocks: usize) -> [Option<usize>; 3] {
    if total_blocks <= 3 {
        let mut targets = [None; 3];
        if current_block < total_blocks {
            targets[0] = Some(current_block);
            let mut idx = 1;
            for i in 0..total_blocks {
                if i != current_block {
                    targets[idx] = Some(i);
                    idx += 1;
                }
            }
        } else {
            for i in 0..total_blocks {
                targets[i] = Some(i);
            }
        }
        targets
    } else if current_block == 0 {
        [Some(0), Some(1), Some(2)]
    } else if current_block + 1 >= total_blocks {
        [Some(total_blocks - 1), Some(total_blocks - 2), Some(total_blocks - 3)]
    } else {
        [Some(current_block), Some(current_block + 1), Some(current_block - 1)]
    }
}

