use core::sync::atomic::Ordering;
use embassy_rp::gpio::{Input, Output};
use pico1541_common::context::ViaHardware;
use pico1541_common::snooper::{IecSnooper, SnooperConfig};
use pico1541_common::util::bit_align_u32;
use pico1541_manager::{EVENT_CHANNEL, TRACK_CHANNEL, CMD_CHANNEL, InternalCommand};
use pico1541_common::interface::HostInterface;
use pico1541_common::DriveEvent;

use super::config::*;
use super::timekeeper::TimeKeeper;

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

    /* FIXME! Disable GPIO debug pins for now.
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_debug_0(&mut self) {
        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(GPIO_DEBUG_MASK_0);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn set_debug_1(&mut self) {
        embassy_rp::pac::SIO.gpio_out(0).value_set().write_value(GPIO_DEBUG_MASK_1);
    }*/

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn clear_debug_pins(&mut self) {
        // FIXME! We toggle the debug pins for now, since we may otherwise miss a pulse.
        //embassy_rp::pac::SIO.gpio_out(0).value_clr().write_value(GPIO_DEBUG_MASK_0 /*| GPIO_DEBUG_MASK_1*/);
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
    fn spin_until(&mut self, target_cycle: u32) {
        if !self.timekeeper.spin_until(target_cycle) {
            // Handle the case where the spin_until function returns false
            embassy_rp::pac::SIO.gpio_out(0).value_xor().write_value(GPIO_DEBUG_MASK_0);
        }
    }
    
    /* FIXME! Unsure if this is needed. Disabled for now
    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn spin_until_half(&mut self, target_cycle: u32) {
        self.timekeeper.spin_until_half(target_cycle);
    }*/

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

        let pb_data_mask = bit_align_u32((via_port_b & Self::VIA1_IEC_MASK_DATA_TX) as u32,
            Self::VIA1_IEC_BIT_DATA_TX, GPIO_IEC_BIT_DATA_TX);

        // IEC_CLK_TX is bit 3, we want it at bit 20 -> shift by 17
        let pb_clk_mask = bit_align_u32((via_port_b & Self::VIA1_IEC_MASK_CLK_TX) as u32,
            Self::VIA1_IEC_BIT_CLK_TX, GPIO_IEC_BIT_CLK_TX);

        // FIXME! Debug pin 1 should not be set when all is functioning normally. DEBUGGING ONLY!
        let target_state = pb_data_mask | pb_clk_mask;
        let mask = GPIO_IEC_MASK_DATA_TX | GPIO_IEC_MASK_CLK_TX;
        let current_state = embassy_rp::pac::SIO.gpio_out(0).value().read();
        let xor_mask = (current_state ^ target_state) & mask;   
        
        embassy_rp::pac::SIO.gpio_out(0).value_xor().write_value(xor_mask | GPIO_DEBUG_MASK_1);
    }

    #[link_section = ".data.ram_func"]
    #[inline(always)]
    fn write_mechanics_port(&mut self, via_port_b: u8) {
        // Map the VIA2 activity LED pin to the RED GPIO LED pin
        let led_bit = bit_align_u32((via_port_b & Self::VIA2_MASK_LED) as u32,
                Self::VIA2_BIT_LED, GPIO_LED_BIT_RED);

        let motor_bit = bit_align_u32((via_port_b & Self::VIA2_MASK_MOTOR) as u32,
                Self::VIA2_BIT_MOTOR, GPIO_LED_BIT_GREEN);

        let led_override = bit_align_u32(led_bit, 
                GPIO_LED_BIT_RED, GPIO_LED_BIT_GREEN);

        let set_mask = led_bit | (motor_bit & !led_override);
        let clr_mask = !set_mask & (GPIO_LED_MASK_RED | GPIO_LED_MASK_GREEN);

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
        embassy_rp::pac::SIO.gpio_in(0).read() & GPIO_IEC_MASK_RESET != 0
    }
}
