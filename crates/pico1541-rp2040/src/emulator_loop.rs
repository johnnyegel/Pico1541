use crate::{HardwareBridge, PioMagneticSurface};
use pico1541_common::interface::HostInterface;
use pico1541_common::interface::IecBus;
use pico1541_common::SYSTEM_CLOCK_FREQ;
use pico1541_emulator::Emulator;

struct Rp2040Timer;

impl Rp2040Timer {
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn time_us(&self) -> u32 {
        embassy_rp::pac::TIMER.timelr().read()
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn syst_current(&self) -> u32 {
        cortex_m::peripheral::SYST::get_current()
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn syst_per_us(&self) -> u32 {
        SYSTEM_CLOCK_FREQ / 1_000_000
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn yield_cpu(&self) {
        cortex_m::asm::nop();
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn halt_delay(&self) {
        cortex_m::asm::delay(100);
    }
}

#[link_section = ".data.ram_func"]
#[inline(never)]
pub fn run(
    emulator: &mut Emulator<HardwareBridge, PioMagneticSurface>,
    hw: &mut HardwareBridge,
    rom_buffer: &'static [u8; 16384],
) -> ! {
    let timer = Rp2040Timer;
    let mut idle_counter = 0u32;
    let mut cycle_counter = 0u32;

    let mut last_tick = timer.time_us();
    let mut was_reset = false;

    let syst_per_us = timer.syst_per_us();
    let mut last_syst = timer.syst_current();
    let mut syst_accum = 0u32;
    let mut now_us = last_tick;

    loop {
        let current_syst = timer.syst_current();
        let diff_syst = last_syst.wrapping_sub(current_syst) & 0x00FFFFFF;
        last_syst = current_syst;

        syst_accum += diff_syst;
        while syst_accum >= syst_per_us {
            syst_accum -= syst_per_us;
            now_us = now_us.wrapping_add(1);
        }
        let now = now_us;

        if hw.check_halt_requested() {
            timer.halt_delay();
            now_us = timer.time_us();
            last_tick = now_us;
            last_syst = timer.syst_current();
            syst_accum = 0;
            continue;
        }

        if hw.consume_trigger_reset() {
            emulator.update_rom(rom_buffer);
            emulator.reset(hw);
        }

        let mut caught_up = 0;
        let is_reset = hw.is_reset_line_asserted();

        while (now.wrapping_sub(last_tick) as i32) > 0 && caught_up < 500 {
            if is_reset {
                last_tick = last_tick.wrapping_add(1);
                cycle_counter = cycle_counter.wrapping_add(1);

                if !was_reset {
                    emulator.reset(hw);
                    hw.iec_drive(pico1541_common::interface::IecBusState::default());
                    hw.send_event(pico1541_common::interface::DriveEvent::LedChanged(false));
                    hw.send_event(pico1541_common::interface::DriveEvent::MotorChanged(false));
                    was_reset = true;
                }
                hw.sync(hw.iec_host());
            } else {
                if was_reset {
                    emulator.reset(hw);
                    hw.send_event(pico1541_common::interface::DriveEvent::LedChanged(true));
                    hw.send_event(pico1541_common::interface::DriveEvent::MotorChanged(true));
                    was_reset = false;
                }

                let elapsed = emulator.tick(hw);
                last_tick = last_tick.wrapping_add(elapsed);
                cycle_counter = cycle_counter.wrapping_add(elapsed as u32);
            }
            caught_up += 1;
        }

        if caught_up == 0 {
            idle_counter = idle_counter.wrapping_add(1);
            timer.yield_cpu();
        }

        // Report exactly on every loop, HostInterface does a fast atomic write
        hw.report_performance_counters(cycle_counter, idle_counter);
    }
}
