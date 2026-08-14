use heapless::String;
use portable_atomic::Ordering;
use crate::driver::{DisplayDriver, FileStorageDriver};
use pico1541_common::snooper::SnooperConfig;
use crate::UiAction;

pub async fn run_monitor_page<D, Internal>(
    display: &mut D,
    internal: &mut Internal,
    config: &SnooperConfig,
) -> bool
where
    D: DisplayDriver,
    Internal: FileStorageDriver,
{
    let free_flash = internal.free_space().await;
    loop {
        display.clear().await;
        display.print(0, "-- System Monitor --").await;

        let speed_hz = config.emu_speed_hz.load(Ordering::Relaxed);
        let c0_avg = config.core0_avg_raw.load(Ordering::Relaxed);
        
        let c1_avg = config.core1_avg_raw.load(Ordering::Relaxed);
        let c1_max = config.core1_max_raw.load(Ordering::Relaxed);

        let mut s_speed = String::<32>::new();
        if speed_hz >= 1_000_000 {
            let mhz_int = speed_hz / 1_000_000;
            let mhz_frac = (speed_hz % 1_000_000) / 10_000;
            let _ = core::fmt::write(&mut s_speed, format_args!("Emu clk : {}.{:02} MHz", mhz_int, mhz_frac));
        } else {
            let _ = core::fmt::write(&mut s_speed, format_args!("Emu clk : {} Hz", speed_hz));
        }
        display.print(1, &s_speed).await;

        let max_syst_cycles = pico1541_common::SYSTEM_CLOCK_FREQ / 50;
        let c0_load = ((c0_avg as u64 * 100) / max_syst_cycles as u64) as u32;
        let mut s_c0 = String::<32>::new();
        let _ = core::fmt::write(&mut s_c0, format_args!("Emu max : {}%", c0_load));
        display.print(2, &s_c0).await;

        let c1_load = if c1_max > 0 {
            100u32.saturating_sub(((c1_avg as u64 * 100) / c1_max as u64) as u32)
        } else {
            0
        };
        let mut s_c1 = String::<32>::new();
        let _ = core::fmt::write(&mut s_c1, format_args!("Mgr load: {}%", c1_load));
        display.print(3, &s_c1).await;

        if let Some(free_bytes) = free_flash {
            let mut s_flash = String::<32>::new();
            let _ = core::fmt::write(&mut s_flash, format_args!("Flash free: {} KB", free_bytes / 1024));
            display.print(4, &s_flash).await;
        }

        display.print(6, " [Back] ").await;
        display.request_redraw();

        match embassy_time::with_timeout(
            embassy_time::Duration::from_millis(200),
            crate::ACTION_CHANNEL.receive()
        ).await {
            Ok(UiAction::Back) => return false,
            Ok(UiAction::Home) => return true,
            _ => {}
        }
    }
}
