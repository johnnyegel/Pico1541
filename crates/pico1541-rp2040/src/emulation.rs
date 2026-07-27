use core::sync::atomic::Ordering;
use embassy_rp::gpio::{Input, Output};
use pico1541_common::context::ViaHardware;
use pico1541_common::snooper::{IecSnooper, SnooperConfig};
use pico1541_common::util::bit_align_u32;
use crate::driver::leds::ActivityLed;
use pico1541_manager::{EVENT_CHANNEL, TRACK_CHANNEL, CMD_CHANNEL, InternalCommand};
use embassy_rp::pac;
use cortex_m::asm::nop;
use pico1541_common::interface::HostInterface;
use crate::PioMagneticSurface;
use pico1541_common::SYSTEM_CLOCK_FREQ;
use pico1541_common::DriveEvent;

use pico1541_emulator::Emulator;

use embassy_rp::multicore::Stack;
use embassy_rp::pio::Pio;
use embassy_rp::peripherals::PIO0;
use embassy_rp::peripherals::CORE1;
use pico1541_common::track_buffer::EmulatorHandle;

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
    stack: &'static mut Stack<8192>,
    hardware: EmulatorHardware,
    config: &'static SnooperConfig,
    drive_id: u8,
    pio0: Pio<'static, PIO0>,
    emulator_handle: EmulatorHandle,
    rom_buffer: &'static [u8; 16384],
) {
    let pins = IecPins {
        _data_rx: embassy_rp::gpio::Input::new(hardware.pin_16, embassy_rp::gpio::Pull::None),
        _clk_rx: embassy_rp::gpio::Input::new(hardware.pin_17, embassy_rp::gpio::Pull::None),
        _atn_rx: embassy_rp::gpio::Input::new(hardware.pin_18, embassy_rp::gpio::Pull::None),
        _data_tx: embassy_rp::gpio::Output::new(hardware.pin_19, embassy_rp::gpio::Level::Low),
        _clk_tx: embassy_rp::gpio::Output::new(hardware.pin_20, embassy_rp::gpio::Level::Low),
        reset_rx: embassy_rp::gpio::Input::new(hardware.pin_21, embassy_rp::gpio::Pull::Up),
    };

    let activity_led = ActivityLed::new(
        embassy_rp::gpio::Output::new(hardware.pin_22, embassy_rp::gpio::Level::Low),
        embassy_rp::gpio::Output::new(hardware.pin_3, embassy_rp::gpio::Level::Low)
    );
    let mut bridge = HardwareBridge::new(pins, config, activity_led,
        embassy_rp::gpio::Output::new(hardware.pin_0, embassy_rp::gpio::Level::Low), embassy_rp::gpio::Output::new(hardware.pin_1, embassy_rp::gpio::Level::Low));

    embassy_rp::multicore::spawn_core1(core1, stack, move || {
        #[link_section = ".sram3"]
        static mut EMULATOR: core::mem::MaybeUninit<Emulator<HardwareBridge, PioMagneticSurface>> = core::mem::MaybeUninit::uninit();

        let drive = unsafe {
            let ptr: *mut Emulator<HardwareBridge, PioMagneticSurface> = core::ptr::addr_of_mut!(EMULATOR).cast();
            let surface = PioMagneticSurface::new(pio0, emulator_handle);
            core::ptr::write(ptr, Emulator::new(rom_buffer, drive_id, surface));
            &mut *ptr
        };
        drive.reset(&mut bridge);
        
        cortex_m::interrupt::disable(); // Guarantee no IRQs preempt the emulator core
        let mut syst = unsafe { cortex_m::Peripherals::steal().SYST };
        syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
        syst.set_reload(0x00FFFFFF);
        syst.clear_current();
        syst.enable_counter();

        run(drive, &mut bridge, rom_buffer);
    });
}

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
            hw.base_time_us = last_tick.wrapping_sub(cycle_counter);
            if is_reset {
                last_tick = last_tick.wrapping_add(1);
                cycle_counter = cycle_counter.wrapping_add(1);

                if !was_reset {
                    emulator.reset(hw);
                    was_reset = true;
                }
            } else {
                if was_reset {
                    emulator.reset(hw);
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
pub(crate) struct IecPins {
    pub(crate) _data_rx: Input<'static>,
    pub(crate) _clk_rx: Input<'static>,
    pub(crate) _atn_rx: Input<'static>,
    pub(crate) _data_tx: Output<'static>,
    pub(crate) _clk_tx: Output<'static>,
    pub(crate) reset_rx: Input<'static>,
}

pub(crate) struct HardwareBridge {
    _pins: IecPins,
    _snooper: IecSnooper,
    config: &'static SnooperConfig,
    activity_led: ActivityLed,
    motor_on: bool,
    dbg_pin_0: Output<'static>,
    dbg_pin_1: Output<'static>,
    pub base_time_us: u32,
}

impl HardwareBridge {
    // Constants for GPIO IEC pin bit position
    const GPIO_IEC_BIT_DATA_RX: usize = 16; // GPIO 16 (PB0)
    const GPIO_IEC_BIT_CLK_RX: usize = 17;  // GPIO 17 (PB2)
    const GPIO_IEC_BIT_ATN_RX: usize = 18;  // GPIO 18 (PB7)
    const GPIO_IEC_BIT_DATA_TX: usize = 19; // GPIO 19 (PB0)
    const GPIO_IEC_BIT_CLK_TX: usize = 20;  // GPIO 20 (PB1)
    //const GPIO_IEC_BIT_RESET: usize = 21;   // GPIO 21 (PB6)

    // Constants for GPIO pin masking
    const GPIO_IEC_MASK_DATA_RX: u32 = 1 << Self::GPIO_IEC_BIT_DATA_RX;
    const GPIO_IEC_MASK_CLK_RX: u32 = 1 << Self::GPIO_IEC_BIT_CLK_RX;
    const GPIO_IEC_MASK_ATN_RX: u32 = 1 << Self::GPIO_IEC_BIT_ATN_RX;
    const GPIO_IEC_MASK_DATA_TX: u32 = 1 << Self::GPIO_IEC_BIT_DATA_TX;
    const GPIO_IEC_MASK_CLK_TX: u32 = 1 << Self::GPIO_IEC_BIT_CLK_TX;
    //const GPIO_IEC_MASK_RESET: u32 = 1 << Self::GPIO_IEC_BIT_RESET;

    // Constants for GPIO LED pins
    const GPIO_LED_BIT_GREEN: usize = 22;  // GPIO 22 (LED)
    const GPIO_LED_BIT_RED: usize = 3;   // GPIO 3 (LED)

    const GPIO_LED_MASK_GREEN: u32 = 1 << Self::GPIO_LED_BIT_GREEN;
    const GPIO_LED_MASK_RED: u32 = 1 << Self::GPIO_LED_BIT_RED;


    pub(crate) fn new(pins: IecPins, config: &'static SnooperConfig, activity_led: ActivityLed, dbg_pin_0: Output<'static>, dbg_pin_1: Output<'static>) -> Self {
        let device_id = config.device_id.load(Ordering::Relaxed);
        let mut bridge = Self {
            _pins: pins,
            _snooper: IecSnooper::new(device_id),
            config,
            activity_led,
            motor_on: true,
            dbg_pin_0,
            dbg_pin_1,
            base_time_us: 0,
        };
        bridge.activity_led.set_state(true, true);
        bridge
    }
}

impl HostInterface for HardwareBridge {
    
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn send_event(&mut self, event: DriveEvent) {
        // try_send is non-blocking and safe for Core 0.
        let _ = EVENT_CHANNEL.try_send(event);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn debug_pin_0(&mut self, state: bool) {
        if state {
            self.dbg_pin_0.set_high();
        } else {
            self.dbg_pin_0.set_low();
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn debug_pin_1(&mut self, state: bool) {
        if state {
            self.dbg_pin_1.set_high();
        } else {
            self.dbg_pin_1.set_low();
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn track_changed(&mut self, half_track: u8) {
        // We notify the TRACK_CHANNEL directly instead of using DriveEvent
        // First we drain the channel to avoid blocking if the manager is behind
        while TRACK_CHANNEL.try_receive().is_ok() {}
        let _ = TRACK_CHANNEL.try_send(half_track);
        
        // Pend TIMER_IRQ_3 on Core 0 to wake up the DiskTask!
        embassy_rp::pac::TIMER.intf().write(|w| w.set_alarm(3, true));
        
        // Use the indicator to beep (from drive_activity context)
        // Actually, indicator is inside activity task, not available here.
        // Wait, the user said: "I would like the HostInterface to have a dedicated track_changed method, which exclusively handles head step events."
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(Ordering::Relaxed)
    }
    
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn pop_discard_request(&mut self) -> bool {
        self.config.discard_request.swap(false, Ordering::Relaxed)
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, Ordering::Relaxed);
        self.config.talk_addr.store(talk, Ordering::Relaxed);
    }

    
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn get_device_id(&self) -> u8 {
        self.config.device_id.load(Ordering::Relaxed)
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn spin_until(&mut self, target_cycle: u32) {
        let target_time = self.base_time_us.wrapping_add(target_cycle);
        while target_time.wrapping_sub(pac::TIMER.timelr().read()) < 0x8000_0000 {
            nop();
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn check_halt_requested(&self) -> bool {
        self.config.halt_emulator.load(Ordering::Relaxed)
    }


    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn report_performance_counters(&mut self, ticks: u32, idles: u32) {
        self.config.emu_total_ticks.store(ticks, Ordering::Relaxed);
        self.config.emu_total_idles.store(idles, Ordering::Relaxed);
    }
}

impl ViaHardware for HardwareBridge {

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn read_iec_port(&mut self) -> u8 {
        // Sample GPIO inputs
        let pins = embassy_rp::pac::SIO.gpio_in(0).read();

        // Align the bits to match the VIA1 input register layout
        let data = bit_align_u32(pins & Self::GPIO_IEC_MASK_DATA_RX, 
            Self::GPIO_IEC_BIT_DATA_RX, Self::VIA1_IEC_BIT_DATA_RX) as u8;
        let clk = bit_align_u32(pins & Self::GPIO_IEC_MASK_CLK_RX, 
            Self::GPIO_IEC_BIT_CLK_RX, Self::VIA1_IEC_BIT_CLK_RX) as u8;
        let atn = bit_align_u32(pins & Self::GPIO_IEC_MASK_ATN_RX, 
            Self::GPIO_IEC_BIT_ATN_RX, Self::VIA1_IEC_BIT_ATN_RX) as u8;

        // Return the combined value in the VIA1 input register format
        data | clk | atn
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write_iec_port(&mut self, via_port_b: u8) {

        let pb_data_mask = bit_align_u32((via_port_b & Self::VIA1_IEC_MASK_DATA_TX) as u32,
            Self::VIA1_IEC_BIT_DATA_TX, Self::GPIO_IEC_BIT_DATA_TX);

        // IEC_CLK_TX is bit 3, we want it at bit 20 -> shift by 17
        let pb_clk_mask = bit_align_u32((via_port_b & Self::VIA1_IEC_MASK_CLK_TX) as u32,
            Self::VIA1_IEC_BIT_CLK_TX, Self::GPIO_IEC_BIT_CLK_TX);

        // FIXME! Disable hijacking for now, as we do not need it yet!
        // let hijack_force = (self.config.should_drive_data.load(Ordering::Relaxed) as u32) << 19;
        let set_mask = pb_data_mask | pb_clk_mask /* | hijack_force */;
        let clr_mask = !set_mask & (Self::GPIO_IEC_MASK_DATA_TX | Self::GPIO_IEC_MASK_CLK_TX);
        
        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(set_mask);
        embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(clr_mask);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write_mechanics_port(&mut self, via_port_b: u8) {
        // Map the VIA2 activity LED pin to the RED GPIO LED pin
        let led_bit = bit_align_u32((via_port_b & Self::VIA2_MASK_LED) as u32,
                Self::VIA2_BIT_LED, Self::GPIO_LED_BIT_RED);

        let motor_bit = bit_align_u32((via_port_b & Self::VIA2_MASK_MOTOR) as u32,
                Self::VIA2_BIT_MOTOR, Self::GPIO_LED_BIT_GREEN);

        let led_override = bit_align_u32(led_bit, 
                Self::GPIO_LED_BIT_RED, Self::GPIO_LED_BIT_GREEN);

        let set_mask = led_bit | (motor_bit & !led_override);
        let clr_mask = !set_mask & (Self::GPIO_LED_MASK_RED | Self::GPIO_LED_MASK_GREEN);

        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(set_mask);
        embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(clr_mask);

        let motor_on_bool = motor_bit != 0;
        if motor_on_bool != self.motor_on {
            self.motor_on = motor_on_bool;
            let _ = CMD_CHANNEL.try_send(InternalCommand::MotorState(motor_on_bool));
        }
    }



    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn consume_trigger_reset(&mut self) -> bool {
        if self.config.trigger_reset.load(Ordering::Relaxed) {
            self.config.trigger_reset.store(false, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn is_reset_line_asserted(&self) -> bool {
        self._pins.reset_rx.is_high()
    }
}
