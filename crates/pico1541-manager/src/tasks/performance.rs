use core::sync::atomic::Ordering;
use pico1541_common::snooper::SnooperConfig;

pub const SYSTEM_CLOCK_FREQ: u32 = 250_000_000;

pub struct Core0PerfTask {
    config: &'static SnooperConfig,
}

impl Core0PerfTask {
    pub fn new(config: &'static SnooperConfig) -> Self {
        Self { config }
    }

    pub async fn run(&mut self) {
        let mut last_emu_ticks = self.config.emu_total_ticks.load(Ordering::Relaxed);
        let mut last_idle_loops = self.config.emu_total_idles.load(Ordering::Relaxed);

        // Start max idle at 1 so it quickly calibrates UP to the actual idle loops
        let mut max_idle_per_20ms = 1u32;
        let mut max_load = 0;

        let mut last_time = embassy_time::Instant::now();
        let mut ticks_since_last = 0;

        loop {
            embassy_time::Timer::after(embassy_time::Duration::from_millis(20)).await;

            let current_emu_ticks = self.config.emu_total_ticks.load(Ordering::Relaxed);
            let current_idle_loops = self.config.emu_total_idles.load(Ordering::Relaxed);

            let diff_ticks = current_emu_ticks.wrapping_sub(last_emu_ticks);
            let diff_idles = current_idle_loops.wrapping_sub(last_idle_loops);

            last_emu_ticks = current_emu_ticks;
            last_idle_loops = current_idle_loops;

            ticks_since_last += diff_ticks;

            if diff_idles > max_idle_per_20ms {
                max_idle_per_20ms = diff_idles;
            }

            if max_idle_per_20ms > 0 {
                let idle_pct = if diff_idles > max_idle_per_20ms {
                    100
                } else {
                    (diff_idles * 100) / max_idle_per_20ms
                };
                let load_pct = 100 - idle_pct;

                // Reconstruct fake SYST cycles for compatibility with main_menu.rs
                let fake_busy_syst = (load_pct * (SYSTEM_CLOCK_FREQ / 50)) / 100;

                if fake_busy_syst > max_load {
                    max_load = fake_busy_syst;
                }
            }

            let now = embassy_time::Instant::now();
            let elapsed_ms = now.duration_since(last_time).as_millis();

            // Update speed and reset stats roughly twice a second
            if elapsed_ms >= 500 {
                let speed_hz = ((ticks_since_last as u64 * 1000) / elapsed_ms) as u32;
                self.config.emu_speed_hz.store(speed_hz, Ordering::Relaxed);
                self.config.core0_avg_raw.store(max_load, Ordering::Relaxed);

                ticks_since_last = 0;
                max_load = 0;
                last_time = now;
            }
        }
    }
}

pub struct Core1LoadTask {
    config: &'static SnooperConfig,
}

impl Core1LoadTask {
    pub fn new(config: &'static SnooperConfig) -> Self {
        Self { config }
    }

    pub async fn run(&mut self) {
        let mut iterations = 0u32;
        let mut last_time = embassy_time::Instant::now();

        loop {
            iterations += 1;
            // Yield to let other tasks run, but don't do it every microsecond!
            // Using a longer delay significantly reduces context switching overhead.
            embassy_time::Timer::after_millis(10).await;

            let now = embassy_time::Instant::now();
            let elapsed = now.duration_since(last_time).as_millis();
            if elapsed >= 1000 {
                let old_avg = self.config.core1_avg_raw.load(Ordering::Relaxed);
                let new_avg = if old_avg == 0 {
                    iterations
                } else {
                    (iterations + old_avg * 3) / 4
                };
                self.config.core1_avg_raw.store(new_avg, Ordering::Relaxed);

                let max_raw = self.config.core1_max_raw.load(Ordering::Relaxed);
                if iterations > max_raw {
                    self.config
                        .core1_max_raw
                        .store(iterations, Ordering::Relaxed);
                }

                iterations = 0;
                last_time = now;
            }
        }
    }
}
