use core::sync::atomic::Ordering;
use embassy_rp::gpio::{Input, Output};
use pico1541_common::context::ViaHardware;
use pico1541_common::snooper::{IecSnooper, SnooperConfig};
use pico1541_common::util::bit_align_u32;
use pico1541_manager::{EVENT_CHANNEL, TRACK_CHANNEL, CMD_CHANNEL, InternalCommand};
use pico1541_common::interface::HostInterface;
use pico1541_common::DriveEvent;
use super::timekeeper::TimeKeeper;

use super::config::*;


pub struct IecPins {
    pub(crate) _data_rx: Input<'static>,
    pub(crate) _clk_rx: Input<'static>,
    pub(crate) _atn_rx: Input<'static>,
    pub(crate) _data_tx: Output<'static>,
    pub(crate) _clk_tx: Output<'static>,
    pub(crate) _reset_rx: Input<'static>,
}

pub struct LedPins {
    pub(crate) _led_pos: Output<'static>,
    pub(crate) _led_neg: Output<'static>,
}

pub struct DebugPins {
    pub(crate) _dbg_0: Output<'static>,
    pub(crate) _dbg_1: Output<'static>,
}

pub struct HardwareBridge {
    pub timekeeper: TimeKeeper,

    _snooper: IecSnooper,
    config: &'static SnooperConfig,
    motor_on: bool,
    _pins_iec: IecPins,
    _pins_led: LedPins,
    _pins_debug: DebugPins,
}

impl HardwareBridge {
    pub fn new(snooper_config: &'static SnooperConfig,
            pins_iec: IecPins, pins_led: LedPins, pins_debug: DebugPins) -> Self {

        let device_id = snooper_config.device_id.load(Ordering::Relaxed);
        let bridge = Self {
            timekeeper: TimeKeeper::new(),
            _snooper: IecSnooper::new(device_id),
            config: snooper_config,
            motor_on: false,
            _pins_iec: pins_iec,
            _pins_led: pins_led,
            _pins_debug: pins_debug
        };
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
    fn track_changed(&mut self, half_track: u8) {
        // We notify the TRACK_CHANNEL directly instead of using DriveEvent
        // First we drain the channel to avoid blocking if the manager is behind
        while TRACK_CHANNEL.try_receive().is_ok() {}
        let _ = TRACK_CHANNEL.try_send(half_track);
        
        // Pend TIMER_IRQ_3 on Core 0 to wake up the DiskTask!
        embassy_rp::pac::TIMER.intf().write(|w| w.set_alarm(3, true));
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn get_device_id(&self) -> u8 {
        self.config.device_id.load(Ordering::Relaxed)
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn spin_until(&mut self, target_cycle: u32) -> bool {
        self.timekeeper.spin_until(target_cycle)
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn spin_until_half(&mut self, target_cycle: u32) -> bool {
        self.timekeeper.spin_until_half(target_cycle)
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
    fn set_reset_state(&mut self, asserted: bool) {
        if asserted {
            // Turn off LEDs when reset is asserted, to indicate the drive is in reset state.
            embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(GPIO_LED_MASK_GREEN | GPIO_LED_MASK_RED);
        } else {
            // Turn on the RED LED when reset is de-asserted, to indicate the drive is ready.
            embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(GPIO_LED_MASK_RED);
        }
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn read_iec_port(&mut self) -> u8 {
        // Sample GPIO inputs
        let pins = embassy_rp::pac::SIO.gpio_in(0).read();

        // Align the bits to match the VIA1 input register layout
        let data = bit_align_u32(pins & GPIO_IEC_MASK_DATA_RX, 
            GPIO_IEC_BIT_DATA_RX, Self::VIA1_IEC_BIT_DATA_RX) as u8;
        let clk = bit_align_u32(pins & GPIO_IEC_MASK_CLK_RX, 
            GPIO_IEC_BIT_CLK_RX, Self::VIA1_IEC_BIT_CLK_RX) as u8;
        let atn = bit_align_u32(pins & GPIO_IEC_MASK_ATN_RX, 
            GPIO_IEC_BIT_ATN_RX, Self::VIA1_IEC_BIT_ATN_RX) as u8;

        // Return the combined value in the VIA1 input register format
        data | clk | atn
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write_iec_port(&mut self, via_port_b: u8) {

        // Check emulator logic state (1 = Asserted, 0 = Released)
        let pb3_asserted = (via_port_b & Self::VIA1_IEC_MASK_CLK_TX) != 0;
        
        // 7405 Inverter Logic:
        // RP2040 Pin HIGH (1) -> 7405 Output LOW (Bus Asserted)
        // RP2040 Pin LOW (0) -> 7405 Output Float (Bus Released)
        
        // SIO Controls CLK_TX (GPIO 20)
        let target_clk = if pb3_asserted { GPIO_IEC_MASK_CLK_TX } else { 0 };
        let mask = GPIO_IEC_MASK_CLK_TX;
        
        // Write to SIO
        embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(mask & !target_clk);
        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(target_clk);

        // PIO Controls DATA_TX (GPIO 19)
        // Extract PB1 (Data) and PB4 (ATN_ACK) as 0 or 1
        let pb1 = (via_port_b >> Self::VIA1_IEC_BIT_DATA_TX) & 1;
        let pb4 = (via_port_b >> Self::VIA1_IEC_BIT_ATN_ACK) & 1;
        
        // Pack into 2-bit command for PIO:
        // Bit 0 (Pin state when ATN pin is 0): pb1
        // Bit 1 (Pin state when ATN pin is 1): pb1 OR NOT pb4
        let pio_cmd = pb1 | ((pb1 | (pb4 ^ 1)) << 1);
        
        embassy_rp::pac::PIO1.txf(0).write_value(pio_cmd as u32);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write_mechanics_port(&mut self, via_port_b: u8) {
        let motor_on = (via_port_b & Self::VIA2_MASK_MOTOR) != 0;
        
        let mut set_mask = 0;
        if (via_port_b & Self::VIA2_MASK_LED) != 0 {
            set_mask = GPIO_LED_MASK_RED;
        } else if motor_on {
            set_mask = GPIO_LED_MASK_GREEN;
        }
        
        let clr_mask = (GPIO_LED_MASK_RED | GPIO_LED_MASK_GREEN) & !set_mask;

        // Clear before set
        embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(clr_mask);
        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(set_mask);

        if motor_on != self.motor_on {
            self.motor_on = motor_on;
            let _ = CMD_CHANNEL.try_send(InternalCommand::MotorState(motor_on));
            
            if motor_on {
                crate::emulation::profiler::start_profiling();
            } else {
                crate::emulation::profiler::snapshot_profiling();
            }
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
        embassy_rp::pac::SIO.gpio_in(0).read() & GPIO_IEC_MASK_RESET != 0
    }
}

pub fn init_iec_pio(mut pio1: embassy_rp::pio::Pio<'static, embassy_rp::peripherals::PIO1>) {
    // IEC DATA TX Logic: Combines PB1, PB4, and ATN in real-time.
    // The emulator pushes a 2-bit state to the TX FIFO representing the physical RP2040 pin state.
    // 1 = HIGH (Drives 7405 LOW = Bus Asserted)
    // 0 = LOW  (Drives 7405 Float = Bus Released)
    // Bit 0: Pin state when ATN pin is 0 (Bus Released)
    // Bit 1: Pin state when ATN pin is 1 (Bus Asserted)
    let iec_prg = pio::pio_asm!(
        r#"
        set pindirs, 1      ; Always Push-Pull (7405 handles Open Collector)
        set x, 0            ; Initialize X to 0 (Release in all cases)
    loop:
        pull noblock        ; If TX FIFO has data, pull to OSR. If empty, OSR = X.
        mov x, osr          ; Store latest state back into X
        jmp pin atn_high    ; Read ATN (GPIO 18). Jump if HIGH (Bus Asserted).
    atn_low:
        mov osr, x          ; OSR = X
        out pins, 1         ; Shift Bit 0 into PINS
        jmp loop
    atn_high:
        mov osr, x          ; OSR = X
        out null, 1         ; Discard Bit 0
        out pins, 1         ; Shift Bit 1 into PINS
        jmp loop
        "#
    ).program;
    
    let iec_loaded = pio1.common.load_program(&iec_prg);
    let data_tx_pin = pio1.common.make_pio_pin(unsafe { embassy_rp::peripherals::PIN_19::steal() });
    let atn_pin = pio1.common.make_pio_pin(unsafe { embassy_rp::peripherals::PIN_18::steal() });
    
    let mut iec_cfg = embassy_rp::pio::Config::default();
    iec_cfg.use_program(&iec_loaded, &[]);
    iec_cfg.set_set_pins(&[&data_tx_pin]);
    iec_cfg.set_out_pins(&[&data_tx_pin]);
    iec_cfg.set_in_pins(&[&atn_pin]);
    iec_cfg.set_jmp_pin(&atn_pin);
    
    // Shift right, auto-pull disabled, 32-bit threshold
    iec_cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Right;
    iec_cfg.shift_out.auto_fill = false;
    
    pio1.sm0.set_config(&iec_cfg);
    pio1.sm0.set_enable(true);
    
    core::mem::forget(data_tx_pin);
    core::mem::forget(atn_pin);
    core::mem::forget(iec_loaded);
    core::mem::forget(pio1); // DO NOT DROP PIO1, otherwise Drop will disable the state machine!
}
