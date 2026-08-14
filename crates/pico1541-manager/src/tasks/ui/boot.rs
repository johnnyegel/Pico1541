use portable_atomic::Ordering;
use crate::driver::{DisplayDriver, RgbLedDriver, StatusLedMode};
use pico1541_common::snooper::SnooperConfig;
use heapless::String;
use embassy_time::{Duration, Timer};
use super::PageState;

pub async fn run_boot_screen<D, L>(
    display: &mut D,
    leds: &mut L,
    config: &SnooperConfig,
    rom_name: &str,
) -> PageState
where
    D: DisplayDriver,
    L: RgbLedDriver,
{
    leds.set_status(StatusLedMode::Unmounted);
    display.clear().await;
    display.print_large(1, "  Pico1541").await; 
    
    let mut id_str = String::<16>::new();
    let _ = core::fmt::write(&mut id_str, format_args!("    Drive ID: {}", config.device_id.load(Ordering::Relaxed)));
    display.print(4, id_str.as_str()).await;
    
    let mut rom_str = String::<64>::new();
    let _ = core::fmt::write(&mut rom_str, format_args!("{}", rom_name));
    display.print(5, rom_str.as_str()).await;
    
    display.request_redraw();
    Timer::after(Duration::from_millis(1500)).await;

    PageState::MainMenu
}
