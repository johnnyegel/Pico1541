use display_interface_i2c::I2CInterface;
use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::mutex::Mutex;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
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

pub struct OledDisplay {
    display: Ssd1306<
        I2CInterface<I2c<'static, I2C0, Async>>,
        DisplaySize128x64,
        BufferedGraphicsMode<DisplaySize128x64>,
    >,
    lines: [heapless::String<64>; 6],
    selected_line: u8,
    scroll_offset: i32,
    scroll_dir: i32,
    dirty: bool,
}

impl OledDisplay {
    pub fn new(i2c: I2c<'static, I2C0, Async>) -> Self {
        let interface = I2CInterface::new(i2c, 0x3C, 0x40);
        let display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

        let lines = [
            heapless::String::new(),
            heapless::String::new(),
            heapless::String::new(),
            heapless::String::new(),
            heapless::String::new(),
            heapless::String::new(),
        ];

        Self {
            display,
            lines,
            selected_line: 0,
            scroll_offset: 0,
            scroll_dir: 1,
            dirty: true,
        }
    }

    pub async fn init(&mut self) {
        self.display.init().unwrap();
        let _ = self.display.clear(BinaryColor::Off);
        let _ = self.display.flush();
    }

    pub fn update_scroll(&mut self) {
        let line = &self.lines[self.selected_line as usize];
        let text_width = line.len() as i32 * 6;

        if text_width > 128 {
            let max_scroll = text_width - 128;
            self.scroll_offset += self.scroll_dir;

            if self.scroll_offset >= max_scroll || self.scroll_offset <= 0 {
                self.scroll_dir = -self.scroll_dir;
            }
            self.dirty = true;
        } else {
            self.scroll_offset = 0;
        }
    }

    #[allow(dead_code)]
    pub fn set_selection(&mut self, index: u8) {
        if index < 6 && self.selected_line != index {
            self.selected_line = index;
            self.scroll_offset = 0;
            self.scroll_dir = 1;
            self.dirty = true;
        }
    }

    pub fn set_line(&mut self, line: u8, text: &str) {
        if line < 6 && self.lines[line as usize] != text {
            self.lines[line as usize].clear();
            let _ = self.lines[line as usize].push_str(text);
            self.dirty = true;
        }
    }

    pub fn clear_lines(&mut self) {
        for line in &mut self.lines {
            line.clear();
        }
        self.dirty = true;
    }

    pub fn redraw(&mut self) {
        if !self.dirty {
            return;
        }

        let _ = self.display.clear(BinaryColor::Off);

        for i in 0..6 {
            let is_selected = i == self.selected_line as usize;
            let y = (i as i32) * 10 + 2;

            let bg_color = if is_selected {
                BinaryColor::On
            } else {
                BinaryColor::Off
            };
            let fg_color = if is_selected {
                BinaryColor::Off
            } else {
                BinaryColor::On
            };

            if is_selected {
                let _ = Rectangle::new(Point::new(0, y - 2), Size::new(128, 11))
                    .into_styled(PrimitiveStyleBuilder::new().fill_color(bg_color).build())
                    .draw(&mut self.display);
            }

            let style = MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(fg_color)
                .build();

            let x_offset = if is_selected { -self.scroll_offset } else { 0 };

            let _ = Text::new(&self.lines[i], Point::new(x_offset, y + 8), style)
                .draw(&mut self.display);
        }

        let _ = self.display.flush();
        self.dirty = false;
    }
}

pub struct DisplayWrapper<M: RawMutex + 'static>(pub &'static Mutex<M, OledDisplay>);

impl<M: RawMutex + 'static> Clone for DisplayWrapper<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: RawMutex + 'static> Copy for DisplayWrapper<M> {}

impl<M: RawMutex + 'static> DisplayDriver for DisplayWrapper<M> {
    async fn clear(&mut self) {
        let mut d = self.0.lock().await;
        d.clear_lines();
    }
    async fn print(&mut self, line: u8, text: &str) {
        let mut d = self.0.lock().await;
        d.set_line(line, text);
    }
    async fn update(&mut self) {
        let mut d = self.0.lock().await;
        d.update_scroll();
    }
    async fn redraw(&mut self) {
        let mut d = self.0.lock().await;
        d.redraw();
    }
    fn request_redraw(&mut self) {
        if let Ok(mut d) = self.0.try_lock() {
            d.dirty = true;
        }
    }
    fn is_dirty(&self) -> bool {
        if let Ok(d) = self.0.try_lock() {
            d.dirty
        } else {
            true // Assume dirty if locked to ensure redraw
        }
    }
}
