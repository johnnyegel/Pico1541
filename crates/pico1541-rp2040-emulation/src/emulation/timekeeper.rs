use cortex_m::peripheral::SYST;
use pico1541_common::SYSTEM_CLOCK_FREQ;

pub struct TimeKeeper {
    pub base_time_24: u32,
    pub idle_counter: u32,
}

impl TimeKeeper {
    #[inline(always)]
    fn now_24() -> u32 {
        // SYST_CVR (Current Value Register) is a 24-bit down counter.
        // We invert it to a 24-bit up counter.
        0x00FFFFFF - SYST::get_current()
    }

    pub fn new() -> Self {
        Self {
            base_time_24: 0,
            idle_counter: 0,
        }
    }

    pub fn init_on_core(&mut self) {
        // Use the cortex_m HAL to safely configure SysTick ON THIS CORE.
        // SysTick is per-core! If `TimeKeeper::new` is called on Core 0 but used
        // on Core 1, Core 1's SysTick will remain disabled and freeze the emulator.
        // We use SystClkSource::Core (250MHz) to ensure it is always ticking.
        let mut syst = unsafe { cortex_m::Peripherals::steal().SYST };
        syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        syst.set_reload(0x00FFFFFF);
        syst.clear_current();
        syst.enable_counter();
        self.base_time_24 = Self::now_24();
    }

    #[inline(always)]
    pub fn update_base(&mut self, current_cycles: u32) {
        // 1 cycle = 1 us. At Core frequency, 1us = (SYSTEM_CLOCK_FREQ / 1_000_000) ticks.
        let current_ticks = current_cycles.wrapping_mul(SYSTEM_CLOCK_FREQ / 1_000_000);
        self.base_time_24 = Self::now_24().wrapping_sub(current_ticks) & 0x00FFFFFF;
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn spin_until(&mut self, target_cycle: u32) -> bool {
        // Use u64 to prevent a discontinuity every 17.18 seconds when target_cycle * 250 overflows u32
        let target_ticks = target_cycle * (SYSTEM_CLOCK_FREQ / 1_000_000);
        let target_time = self.base_time_24.wrapping_add(target_ticks) & 0x00FFFFFF;

        let mut did_wait = false;
        while target_time.wrapping_sub(Self::now_24()) & 0x00FFFFFF < 0x800000 {
            self.idle_counter = self.idle_counter.wrapping_add(1);
            //cortex_m::asm::nop();
            did_wait = true;
        }
        did_wait
    }

    /* FIXME! Unsure if this is needed. Disabled for now
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    pub fn spin_until_half(&mut self, target_cycle: u32) {
        let target_ticks = target_cycle.wrapping_mul(SYSTEM_CLOCK_FREQ / 1_000_000);
        let target_time = self.base_time_24.wrapping_add(target_ticks) & 0x00FFFFFF;

        while target_time.wrapping_sub(Self::now_24()) & 0x00FFFFFF < 0x800000 {
            self.idle_counter = self.idle_counter.wrapping_add(1);
            cortex_m::asm::nop();
        }
    } */
}
