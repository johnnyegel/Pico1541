pub mod config;
pub mod timekeeper;
pub mod profiler;
pub mod bridge;

use pico1541_common::snooper::SnooperConfig;
use pico1541_emulator::{DriveMechanics, Emulator};
use pico1541_common::context::ViaHardware;

use embassy_rp::multicore::Stack;
use embassy_rp::pio::Pio;
use embassy_rp::peripherals::PIO0;
use embassy_rp::peripherals::CORE1;

use pico1541_common::track_buffer::EmulatorHandle;
use crate::magnetic_surface::PioMagneticSurface;
use bridge::{HardwareBridge, IecPins, LedPins, DebugPins};

pub struct EmulatorHardware {
    pub pin_16: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_16>,
    pub pin_17: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_17>,
    pub pin_18: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_18>,
    pub pin_19: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_19>,
    pub pin_20: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_20>,
    pub pin_21: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_21>,
    pub pin_22: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_22>,
    pub pin_3: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_3>,
    pub pin_0: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_0>,
    pub pin_1: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_1>,
}

pub fn spawn_core1(
    core1: embassy_rp::Peri<'static, CORE1>,
    stack: &'static mut Stack<1024>,
    hardware: EmulatorHardware,
    snooper_config: &'static SnooperConfig,
    drive_id: u8,
    pio0: Pio<'static, PIO0>,
    emulator_handle: EmulatorHandle,
    rom_buffer: &'static [u8; 16384],
) {
    let pins_iec = IecPins {
        _data_rx: embassy_rp::gpio::Input::new(hardware.pin_16, embassy_rp::gpio::Pull::None),
        _clk_rx: embassy_rp::gpio::Input::new(hardware.pin_17, embassy_rp::gpio::Pull::None),
        _atn_rx: embassy_rp::gpio::Input::new(hardware.pin_18, embassy_rp::gpio::Pull::None),
        _data_tx: embassy_rp::gpio::Output::new(hardware.pin_19, embassy_rp::gpio::Level::Low),
        _clk_tx: embassy_rp::gpio::Output::new(hardware.pin_20, embassy_rp::gpio::Level::Low),
        _reset_rx: embassy_rp::gpio::Input::new(hardware.pin_21, embassy_rp::gpio::Pull::Up),
    };

    let pins_led = LedPins {
        _led_pos: embassy_rp::gpio::Output::new(hardware.pin_22, embassy_rp::gpio::Level::Low),
        _led_neg: embassy_rp::gpio::Output::new(hardware.pin_3, embassy_rp::gpio::Level::Low),
    };

    let pins_debug = DebugPins {
        _dbg_0: embassy_rp::gpio::Output::new(hardware.pin_0, embassy_rp::gpio::Level::Low),
        _dbg_1: embassy_rp::gpio::Output::new(hardware.pin_1, embassy_rp::gpio::Level::Low),    
    };

    let mut bridge = HardwareBridge::new(snooper_config, pins_iec, pins_led, pins_debug);

    embassy_rp::multicore::spawn_core1(core1, stack, move || {
        static mut EMULATOR: core::mem::MaybeUninit<Emulator<HardwareBridge, PioMagneticSurface>> = core::mem::MaybeUninit::uninit();
        #[link_section = ".sram3"]
        static mut SYS_MEMORY: [u8; 32768] = [0; 32768];

        defmt::info!("Core 1 spawned! Initializing emulator...");

        // :: Set Core1 to have bus priority
        // Disable interrupts on Core1 to prevent preemption of the emulator core
        embassy_rp::pac::BUSCTRL.bus_priority().write(|w| w.set_proc1(true));
        cortex_m::interrupt::disable(); // Guarantee no IRQs preempt the emulator core

        // :: Initialize the emulator in place in SRAM3
        // We do not want this on the stack, because it is too large and will cause stack overflow.
        let emulator = unsafe {
            let ptr: *mut Emulator<HardwareBridge, PioMagneticSurface> = core::ptr::addr_of_mut!(EMULATOR).cast();
            let memory = &mut *core::ptr::addr_of_mut!(SYS_MEMORY);
            let surface = PioMagneticSurface::new(pio0);
            let mechanics = DriveMechanics::new(surface, emulator_handle);
            core::ptr::write(ptr, Emulator::new(rom_buffer, memory, drive_id, mechanics));
            &mut *ptr
        };
        
        defmt::info!("Emulator initialized. Starting run loop...");
        crate::emulation::profiler::configure_sram_contention_counters();

        run_emulator(emulator, &mut bridge, rom_buffer);
    });
}

#[link_section = ".data.ram_func"]
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
            
            // :: Advance the emulator by 512 ticks
            for _ in 0..512 {
                emulator.tick(hw);
            }
        }

        // Report exactly on every loop, HostInterface does a fast atomic write
        hw.report_performance_counters(emulator.cycles, hw.timekeeper.idle_counter);
    }
}
