use display_interface_i2c::I2CInterface;
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::Mutex;
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoTextStyleBuilder,
    },
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::Text,
};
use pico1541_manager::DisplayDriver;
use ssd1306::{
    mode::BufferedGraphicsMode, prelude::*, rotation::DisplayRotation, size::DisplaySize128x64,
    Ssd1306,
};

#[derive(Clone)]
pub struct DisplayState {
    pub lines: [heapless::String<64>; 6],
    pub large_lines: u8,
    pub selected_line: u8,
    pub scroll_offset: i32,
    pub scroll_dir: i32,
    pub dirty: bool,
    pub needs_init: bool,
}

impl DisplayState {
    pub fn new() -> Self {
        Self {
            lines: [
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
                heapless::String::new(),
            ],
            large_lines: 0,
            selected_line: 0,
            scroll_offset: 0,
            scroll_dir: 1,
            dirty: true,
            needs_init: false,
        }
    }

    pub fn update_scroll(&mut self) {
        let line = &self.lines[self.selected_line as usize];
        let is_large = (self.large_lines >> self.selected_line) & 1 != 0;
        let char_width = if is_large { 10 } else { 6 };
        let text_width = line.len() as i32 * char_width;

        if text_width > 128 {
            let max_scroll = text_width - 128;
            self.scroll_offset += self.scroll_dir * 3;

            if self.scroll_offset >= max_scroll {
                self.scroll_offset = max_scroll;
                self.scroll_dir = -1;
            } else if self.scroll_offset <= 0 {
                self.scroll_offset = 0;
                self.scroll_dir = 1;
            }
            self.dirty = true;
        } else {
            self.scroll_offset = 0;
        }
    }

    pub fn set_selection(&mut self, index: u8) {
        if index < 6 && self.selected_line != index {
            self.selected_line = index;
            self.scroll_offset = 0;
            self.scroll_dir = 1;
            self.dirty = true;
        }
    }

    pub fn set_line(&mut self, line: u8, text: &str, large: bool) {
        if line < 6 {
            if self.lines[line as usize] != text || ((self.large_lines >> line) & 1 != 0) != large {
                self.lines[line as usize].clear();
                let _ = self.lines[line as usize].push_str(text);
                if large {
                    self.large_lines |= 1 << line;
                } else {
                    self.large_lines &= !(1 << line);
                }
                self.dirty = true;
            }
        }
    }

    pub fn clear_lines(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.large_lines = 0;
        self.dirty = true;
    }
}

pub struct DisplayHardware {
    display: Ssd1306<
        I2CInterface<I2c<'static, I2C0, Async>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    retry_counter: u8,
}

impl DisplayHardware {
    pub fn new(i2c: I2c<'static, I2C0, Async>) -> Self {
        let interface = I2CInterface::new(i2c, 0x3C, 0x40);
        let display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        Self {
            display,
            retry_counter: 0,
        }
    }

    pub fn init(&mut self) -> bool {
        if self.display.init().is_err() {
            defmt::error!("OLED display init failed");
            false
        } else {
            let _ = self
                .display
                .clear(embedded_graphics::pixelcolor::BinaryColor::Off);
            let _ = self.display.flush();
            true
        }
    }

    pub fn redraw(&mut self, state: &DisplayState) -> bool {
        if state.needs_init {
            if self.retry_counter > 0 {
                self.retry_counter -= 1;
                return false;
            }

            defmt::warn!("OLED: Re-initializing display due to needs_init flag...");
            if !self.init() {
                defmt::error!("OLED: Re-initialization failed! Retrying in 2 seconds...");
                self.retry_counter = 40; // 40 loops at ~50ms per loop = ~2 seconds
                return false;
            }
            defmt::info!("OLED: Re-initialization successful.");
            self.retry_counter = 0;
        }

        if !state.dirty {
            return true;
        }

        if self.display.clear(BinaryColor::Off).is_err() {
            defmt::error!("OLED: display.clear() failed during redraw!");
            return false;
        }

        for i in 0..6 {
            let is_large = (state.large_lines >> i) & 1 != 0;
            let is_selected = i == state.selected_line as usize;
            let is_header = i == 0;

            let y = (i as i32) * 10 + 2;

            let bg_color = if is_header {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            let fg_color = if is_header {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };

            if is_header {
                let _ = Rectangle::new(Point::new(0, y - 2), Size::new(128, 11))
                    .into_styled(PrimitiveStyleBuilder::new().fill_color(bg_color).build())
                    .draw(&mut self.display);
            }

            let font = if is_large { &FONT_10X20 } else { &FONT_6X10 };
            let char_width = if is_large { 10 } else { 6 };
            let style = MonoTextStyleBuilder::new()
                .font(font)
                .text_color(fg_color)
                .background_color(bg_color)
                .build();

            let y_adj = if is_large { 15 } else { 8 };

            let mut text_y = y + y_adj;
            if is_header {
                text_y -= 2;
            }

            let text = &state.lines[i];
            if is_selected && text.len() >= 2 && text.is_char_boundary(2) {
                let (prefix, rest) = text.split_at(2);

                let rest_x = 2 * char_width - state.scroll_offset;
                let _ = Text::new(rest, Point::new(rest_x, text_y), style).draw(&mut self.display);

                let _ = Text::new(prefix, Point::new(0, text_y), style).draw(&mut self.display);
            } else {
                let x_offset = if is_selected { -state.scroll_offset } else { 0 };
                let _ =
                    Text::new(text, Point::new(x_offset, text_y), style).draw(&mut self.display);
            }
        }

        if self.display.flush().is_ok() {
            true
        } else {
            defmt::error!("OLED: display.flush() failed during redraw!");
            false
        }
    }
}

pub struct DisplayWrapper<M: RawMutex + 'static> {
    pub state: &'static Mutex<M, DisplayState>,
    pub hw: &'static Mutex<M, DisplayHardware>,
}

impl<M: RawMutex + 'static> Clone for DisplayWrapper<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: RawMutex + 'static> Copy for DisplayWrapper<M> {}

impl<M: RawMutex + 'static> DisplayDriver for DisplayWrapper<M> {
    async fn clear(&mut self) {
        let mut d = self.state.lock().await;
        d.clear_lines();
    }
    async fn print(&mut self, line: u8, text: &str) {
        let mut d = self.state.lock().await;
        d.set_line(line, text, false);
    }
    async fn print_large(&mut self, line: u8, text: &str) {
        let mut d = self.state.lock().await;
        d.set_line(line, text, true);
    }
    async fn update(&mut self) {
        let mut d = self.state.lock().await;
        d.update_scroll();
    }
    async fn redraw(&mut self) {
        let state_copy = {
            let state = self.state.lock().await;
            if !state.dirty && !state.needs_init {
                return;
            }
            state.clone()
        };

        let success = {
            let mut hw = self.hw.lock().await;
            hw.redraw(&state_copy)
        };

        let mut state = self.state.lock().await;
        if success {
            // Only clear the dirty flag if the state hasn't been modified in the meantime.
            // But checking for modifications is complex. Just clear it. If it was modified,
            // it might have missed a frame, but the next UI interaction will set it again.
            // Actually, we can check if it matches the copy (or at least if dirty was set).
            // A simpler way: we just set needs_init on failure.
            state.needs_init = false;
            state.dirty = false;
        } else {
            state.needs_init = true;
        }
    }
    fn request_redraw(&mut self) {
        if let Ok(mut d) = self.state.try_lock() {
            d.dirty = true;
        }
    }
    fn is_dirty(&self) -> bool {
        if let Ok(d) = self.state.try_lock() {
            if d.dirty || d.needs_init {
                return true;
            }
            let line = &d.lines[d.selected_line as usize];
            let is_large = (d.large_lines >> d.selected_line) & 1 != 0;
            let char_width = if is_large { 10 } else { 6 };
            let text_width = line.len() as i32 * char_width;
            text_width > 128
        } else {
            true // Assume dirty if locked to ensure redraw
        }
    }
    fn set_selected_line(&mut self, line: u8) {
        if let Ok(mut d) = self.state.try_lock() {
            d.set_selection(line);
        }
    }
}
