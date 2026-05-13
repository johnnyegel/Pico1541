use crate::driver::{DisplayDriver, RgbLedDriver, IndicatorDriver, ActivityColor};
use crate::KEYBOARD_CHANNEL;
use embassy_time::{Duration, Timer, Instant};
use heapless::String;
use core::fmt::Write;
use embassy_futures::select::{select, Either};

pub struct HardwareTestTask<D, L, Ind>
where
    D: DisplayDriver,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
{
    display: D,
    leds: L,
    indicator: Ind,
}

impl<D, L, Ind> HardwareTestTask<D, L, Ind>
where
    D: DisplayDriver,
    L: RgbLedDriver,
    Ind: IndicatorDriver,
{
    pub fn new(display: D, leds: L, indicator: Ind) -> Self {
        Self {
            display,
            leds,
            indicator,
        }
    }

    pub async fn run(&mut self) {
        self.display.clear().await;
        self.display.print(0, "Hardware Test Mode").await;
        self.display.print(1, "Buttons: 0000").await;
        self.display.print(4, "Hold BACK to exit").await;
        self.display.request_redraw();
        
        let mut step = 0;
        let mut next_led_update = Instant::now();
        let mut back_pressed_since: Option<Instant> = None;

        loop {
            let now = Instant::now();
            
            // Check if we should exit (2 seconds hold)
            if let Some(pressed_at) = back_pressed_since {
                if now - pressed_at >= Duration::from_secs(2) {
                    return; // Exit task
                }
            }

            let led_timer = if next_led_update <= now {
                Timer::after(Duration::from_ticks(0))
            } else {
                Timer::at(next_led_update)
            };
            
            let kbd_event = KEYBOARD_CHANNEL.receive();
            
            match select(led_timer, kbd_event).await {
                Either::First(_) => {
                    // LED update interval
                    step += 1;
                    next_led_update = Instant::now() + Duration::from_millis(500);
                    
                    // 1. Cycle Status RGB LEDs
                    let (r, g, b) = match step % 4 {
                        0 => (true, false, false),
                        1 => (false, true, false),
                        2 => (false, false, true),
                        _ => (false, false, false),
                    };
                    self.leds.set_color(r, g, b);

                    // 2. Cycle Activity LED
                    let act_color = match step % 3 {
                        0 => ActivityColor::Red,
                        1 => ActivityColor::Green,
                        _ => ActivityColor::Off,
                    };
                    self.indicator.set_activity_led(act_color);

                    // 4. Update Display Text
                    let s_color = match step % 4 {
                        0 => "RED",
                        1 => "GREEN",
                        2 => "BLUE",
                        _ => "OFF",
                    };
                    let mut st_line = String::<32>::new();
                    let _ = write!(st_line, "Status: {}", s_color);
                    self.display.print(2, &st_line).await;

                    let a_color = match step % 3 {
                        0 => "RED",
                        1 => "GREEN",
                        _ => "OFF",
                    };
                    let mut act_line = String::<32>::new();
                    let _ = write!(act_line, "Activity: {}", a_color);
                    self.display.print(3, &act_line).await;
                    
                    self.display.request_redraw();
                }
                Either::Second(buttons) => {
                    // Keyboard event
                    let mut status = String::<32>::new();
                    let _ = write!(status, "Buttons: {:04b}", buttons);
                    self.display.print(1, &status).await;
                    
                    // Update Back button hold state
                    if buttons & 0b0001 != 0 {
                        if back_pressed_since.is_none() {
                            back_pressed_since = Some(Instant::now());
                        }
                    } else {
                        back_pressed_since = None;
                    }
                    
                    self.display.request_redraw();
                }
            }
        }
    }
}
