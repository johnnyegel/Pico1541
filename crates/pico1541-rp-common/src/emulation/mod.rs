pub mod bridge;
pub mod config;
pub mod magnetic_surface;
pub mod profiler;
pub mod timekeeper;

use pico1541_common::context::ViaHardware;
use pico1541_common::snooper::SnooperConfig;
use pico1541_emulator::{DriveMechanics, Emulator};

use embassy_rp::multicore::Stack;
use embassy_rp::peripherals::{CORE1, PIO0, PIO1};
use embassy_rp::pio::Pio;

use crate::emulation::magnetic_surface::PioMagneticSurface;
use bridge::{DebugPins, HardwareBridge, IecPins, LedPins};
use pico1541_common::track_buffer::EmulatorHandle;

pub struct EmulatorHardware {}

#[no_mangle]
pub static mut RAM_POINTER: *mut [u8; 32768] = core::ptr::null_mut();

pub fn spawn_core1(
    core1: embassy_rp::Peri<'static, CORE1>,
    stack: &'static mut Stack<1024>,
    _hardware: EmulatorHardware,
    snooper_config: &'static SnooperConfig,
    drive_id: u8,
    pio0: Pio<'static, PIO0>,
    pio1: Pio<'static, PIO1>,
    emulator_handle: EmulatorHandle,
    rom_buffer: &'static [u8; 16384],
    sys_clock_freq: u32,
) {
    use crate::emulation::config::*;
    let pins_iec = IecPins {
        _data_rx: embassy_rp::gpio::Input::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_IEC_BIT_DATA_RX as u8) },
            embassy_rp::gpio::Pull::None,
        ),
        _clk_rx: embassy_rp::gpio::Input::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_IEC_BIT_CLK_RX as u8) },
            embassy_rp::gpio::Pull::None,
        ),
        _atn_rx: embassy_rp::gpio::Input::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_IEC_BIT_ATN_RX as u8) },
            embassy_rp::gpio::Pull::None,
        ),
        _reset_rx: embassy_rp::gpio::Input::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_IEC_BIT_RESET as u8) },
            embassy_rp::gpio::Pull::None,
        ),
    };

    let pins_led = LedPins {
        _led_pos: embassy_rp::gpio::Output::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_LED_BIT_GREEN as u8) },
            embassy_rp::gpio::Level::Low,
        ),
        _led_neg: embassy_rp::gpio::Output::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_LED_BIT_RED as u8) },
            embassy_rp::gpio::Level::Low,
        ),
    };

    let pins_debug = DebugPins {
        _dbg_0: embassy_rp::gpio::Output::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_DEBUG_BIT_0 as u8) },
            embassy_rp::gpio::Level::Low,
        ),
        _dbg_1: embassy_rp::gpio::Output::new(
            unsafe { embassy_rp::gpio::AnyPin::steal(GPIO_DEBUG_BIT_1 as u8) },
            embassy_rp::gpio::Level::Low,
        ),
    };

    bridge::init_iec_pio(pio1);

    let mut bridge = HardwareBridge::new(
        snooper_config,
        pins_iec,
        pins_led,
        pins_debug,
        sys_clock_freq,
    );

    embassy_rp::multicore::spawn_core1(core1, stack, move || {
        #[link_section = ".core1_fast_bss"]
        static mut EMULATOR: core::mem::MaybeUninit<Emulator<HardwareBridge, PioMagneticSurface>> =
            core::mem::MaybeUninit::uninit();

        #[link_section = ".core1_uninit"]
        static mut SYS_MEMORY: [u8; 32768] = [0; 32768];

        defmt::info!("Core 1 spawned! Initializing emulator...");

        // :: Set Core1 to have bus priority
        embassy_rp::pac::BUSCTRL
            .bus_priority()
            .write(|w| w.set_proc1(true));

        // Note: Do NOT disable interrupts on Core 1!
        // The embassy_rp flash driver relies on SIO_IRQ_PROC1 to pause Core 1
        // during flash erase/write operations. If interrupts are disabled,
        // Core 0 will deadlock waiting for Core 1 to acknowledge the pause!

        // :: Initialize the emulator in place in SRAM3
        // We do not want this on the stack, because it is too large and will cause stack overflow.
        let emulator = unsafe {
            let ptr: *mut Emulator<HardwareBridge, PioMagneticSurface> =
                core::ptr::addr_of_mut!(EMULATOR).cast();
            let memory = &mut *core::ptr::addr_of_mut!(SYS_MEMORY);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(RAM_POINTER), memory as *mut _);
            let surface = PioMagneticSurface::new(pio0, sys_clock_freq);
            let mechanics = DriveMechanics::new(surface, emulator_handle);
            core::ptr::write(ptr, Emulator::new(rom_buffer, memory, drive_id, mechanics));
            &mut *ptr
        };

        defmt::info!("Emulator initialized. Starting run loop...");
        crate::emulation::profiler::configure_sram_contention_counters();

        run_emulator(emulator, &mut bridge, rom_buffer);
    });
}

#[cold]
#[inline(never)]
#[link_section = ".core1_text"]
fn pause_emulator(hw: &mut HardwareBridge, cycles: u32) {
    use pico1541_common::interface::HostInterface;
    hw.assert_iec_data_busy(true);
    while hw.check_halt_requested() {
        cortex_m::asm::nop();
    }
    hw.assert_iec_data_busy(false);
    hw.timekeeper.update_base(cycles);
}

#[link_section = ".core1_text"]
#[inline(never)]
pub fn run_emulator(
    emulator: &mut Emulator<HardwareBridge, PioMagneticSurface>,
    hw: &mut HardwareBridge,
    rom_buffer: &'static [u8; 16384],
) -> ! {
    use pico1541_common::interface::HostInterface;

    hw.timekeeper.init_on_core();

    let mut was_reset = true;
    hw.timekeeper.update_base(emulator.cycles);

    loop {
        let is_reset = if hw.consume_trigger_reset() {
            emulator.update_rom(rom_buffer);
            true
        } else {
            hw.is_reset_line_asserted()
        };

        // :? Are we in reset?
        if is_reset {
            // -> Yes, then clear the was reset flag, and ensure the timekeeper is updated.
            if !was_reset {
                hw.set_reset_state(true);
                was_reset = true;
            }
            // Keep track of time even in reset state.
            hw.timekeeper.update_base(emulator.cycles);
        } else {
            // -> No :? Was the reset line previously asserted?
            if was_reset {
                // -> Yes, then clear the was reset flag, and reset the emulator.
                hw.set_reset_state(false);
                emulator.reset(hw);
                hw.timekeeper.update_base(emulator.cycles);
                was_reset = false;
            }

            if hw.check_halt_requested() {
                pause_emulator(hw, emulator.cycles);
            }

            // :: Advance the emulator by 512 ticks
            for _ in 0..512 {
                emulator.tick(hw);
            }
            emulator.poll_revolution();
        }

        // Report exactly on every loop, HostInterface does a fast atomic write
        hw.report_performance_counters(emulator.cycles, hw.timekeeper.idle_counter);
    }
}
