use super::timekeeper::TimeKeeper;
use crate::driver::gpio::Gpio;
use core::sync::atomic::Ordering;
use embassy_rp::gpio::{Input, Output};
use pico1541_common::context::ViaHardware;
use pico1541_common::interface::HostInterface;
use pico1541_common::snooper::{IecSnooper, SnooperConfig};
use pico1541_common::util::bit_align_u32;
use pico1541_common::DriveEvent;
use pico1541_manager::{InternalCommand, CMD_CHANNEL};

use super::config::*;

pub struct IecPins {
    pub(crate) _data_rx: Input<'static>,
    pub(crate) _clk_rx: Input<'static>,
    pub(crate) _atn_rx: Input<'static>,
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
    led_gpio: Gpio,
    iec_reset_gpio: Gpio,
    last_via1_port_b: u8,
}

impl HardwareBridge {
    pub fn new(
        snooper_config: &'static SnooperConfig,
        pins_iec: IecPins,
        pins_led: LedPins,
        pins_debug: DebugPins,
        sys_clock_freq: u32,
    ) -> Self {
        let device_id = snooper_config.device_id.load(Ordering::Relaxed);
        let bridge = Self {
            timekeeper: TimeKeeper::new(sys_clock_freq),
            _snooper: IecSnooper::new(device_id),
            config: snooper_config,
            motor_on: false,
            _pins_iec: pins_iec,
            _pins_led: pins_led,
            _pins_debug: pins_debug,
            led_gpio: Gpio::new(GPIO_LED_MASK_GREEN | GPIO_LED_MASK_RED),
            iec_reset_gpio: Gpio::new(GPIO_IEC_MASK_RESET),
            last_via1_port_b: 0,
        };
        bridge
    }

    #[link_section = ".core1_text"]
    #[inline(never)]
    pub fn assert_iec_data_busy(&mut self, busy: bool) {
        if busy {
            // Force PB1 (DATA_OUT) = 1 (asserted low on bus) while preserving CLK and ATN_ACK
            let clk_out = (self.last_via1_port_b >> pico1541_common::context::via::VIA1_IEC_BIT_CLK_TX) & 1;
            let pb4 = (self.last_via1_port_b >> (pico1541_common::context::via::VIA1_IEC_BIT_ATN_ACK - 2)) & 4;
            let pio_cmd = clk_out | 2 | pb4; // PB1 = 1 (bit 1 = 2)
            embassy_rp::pac::PIO1.txf(0).write_value(pio_cmd as u32);
        } else {
            self.write_iec_port(self.last_via1_port_b);
        }
    }
}

impl HostInterface for HardwareBridge {
    #[link_section = ".core1_text"]
    #[inline(always)]
    fn send_event(&mut self, _event: DriveEvent) {
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(Ordering::Relaxed)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn pop_discard_request(&mut self) -> bool {
        self.config.discard_request.swap(false, Ordering::Relaxed)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, Ordering::Relaxed);
        self.config.talk_addr.store(talk, Ordering::Relaxed);
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn track_changed(&mut self, half_track: u8) {
        RpTrackHandoff::new().notify_track_change(half_track);
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn get_device_id(&self) -> u8 {
        self.config.device_id.load(Ordering::Relaxed)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn spin_until(&mut self, target_cycle: u32) -> bool {
        self.timekeeper.spin_until(target_cycle)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn spin_until_half_cycle(&mut self, target_cycle: u32) -> bool {
        self.timekeeper.spin_until_half_cycle(target_cycle)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn intercept_cd_command(&mut self, path: &str) {
        let mut s = heapless::String::<64>::new();
        let _ = s.push_str(path);
        let _ = pico1541_manager::CMD_CHANNEL.try_send(pico1541_manager::InternalCommand::Cd(s));
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn select_virtual_file(&mut self, slot: usize) {
        if pico1541_common::VIRTUAL_FS_ACTIVE.load(Ordering::Relaxed) {
            let _ = pico1541_manager::CMD_CHANNEL.try_send(pico1541_manager::InternalCommand::SelectVirtualFile(slot));
        }
    }


    #[link_section = ".core1_text"]
    #[inline(always)]
    fn check_halt_requested(&self) -> bool {
        pico1541_common::HALT_EMULATOR.load(Ordering::Acquire)
            || self.config.halt_emulator.load(Ordering::Acquire)
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn report_performance_counters(&mut self, ticks: u32, idles: u32) {
        self.config.emu_total_ticks.store(ticks, Ordering::Relaxed);
        self.config.emu_total_idles.store(idles, Ordering::Relaxed);
    }
}

impl ViaHardware for HardwareBridge {
    #[link_section = ".core1_text"]
    #[inline(always)]
    fn set_reset_state(&mut self, asserted: bool) {
        if asserted {
            // Turn off LEDs when reset is asserted, to indicate the drive is in reset state.
            self.led_gpio.write(0);
        } else {
            // Turn on the RED LED when reset is de-asserted, to indicate the drive is ready.
            self.led_gpio.write(GPIO_LED_MASK_RED);
        }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn read_iec_port(&mut self) -> u8 {
        // Sample GPIO inputs
        let pins = Gpio::read_all();

        // Align the bits to match the VIA1 input register layout
        let data = bit_align_u32(
            pins & GPIO_IEC_MASK_DATA_RX,
            GPIO_IEC_BIT_DATA_RX,
            pico1541_common::context::via::VIA1_IEC_BIT_DATA_RX,
        ) as u8;
        let clk = bit_align_u32(
            pins & GPIO_IEC_MASK_CLK_RX,
            GPIO_IEC_BIT_CLK_RX,
            pico1541_common::context::via::VIA1_IEC_BIT_CLK_RX,
        ) as u8;
        let atn = bit_align_u32(
            pins & GPIO_IEC_MASK_ATN_RX,
            GPIO_IEC_BIT_ATN_RX,
            pico1541_common::context::via::VIA1_IEC_BIT_ATN_RX,
        ) as u8;

        // Return the combined value in the VIA1 input register format
        data | clk | atn
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn write_iec_port(&mut self, via_port_b: u8) {
        self.last_via1_port_b = via_port_b;

        // PIO Unified IEC Logic
        // Bit 0: CLK target
        // Bit 1: PB1 (DATA_OUT)
        // Bit 2: PB4 (ATNA)

        // NOTE! We align and mask the bits to their target positions in the PIO command format
        let clk_out = (via_port_b >> pico1541_common::context::via::VIA1_IEC_BIT_CLK_TX) & 1;
        let pb1 = (via_port_b >> (pico1541_common::context::via::VIA1_IEC_BIT_DATA_TX - 1)) & 2;
        let pb4 = (via_port_b >> (pico1541_common::context::via::VIA1_IEC_BIT_ATN_ACK - 2)) & 4;

        // Combine the bits into a single command for the PIO state machine
        let pio_cmd = clk_out | pb1 | pb4;

        embassy_rp::pac::PIO1.txf(0).write_value(pio_cmd as u32);
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn write_mechanics_port(&mut self, via_port_b: u8) {
        let motor_on = (via_port_b & pico1541_common::context::via::VIA2_MASK_MOTOR) != 0;
        let act_on = (via_port_b & pico1541_common::context::via::VIA2_MASK_ACTLED) != 0;

        let mut set_mask = 0;
        if act_on {
            set_mask = GPIO_LED_MASK_RED;
        } else if motor_on {
            set_mask = GPIO_LED_MASK_GREEN;
        }

        self.led_gpio.write(set_mask);

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

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn consume_trigger_reset(&mut self) -> bool {
        if self.config.trigger_reset.load(Ordering::Relaxed) {
            self.config.trigger_reset.store(false, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    #[link_section = ".core1_text"]
    #[inline(always)]
    fn is_reset_line_asserted(&self) -> bool {
        self.iec_reset_gpio.read() != 0
    }
}

pub fn init_iec_pio(mut pio1: embassy_rp::pio::Pio<'static, embassy_rp::peripherals::PIO1>) {
    // PIO unified IEC logic for CLK and DATA
    // Bit 0: CLK target
    // Bit 1: PB1 (DATA_OUT)
    // Bit 2: PB4 (ATNA)
    let iec_prg = pio::pio_asm!(
        r#"
        set x, 0            ; Initialize X to 0 (Release in all cases)
    loop:
        ; 1. Fetch the unified VIA state
        pull noblock        ; Pull new state from Rust, or restore previous state from X
        mov x, osr          ; Back up the state in X for the next loop
        
        ; 2. Handle CLOCK (Bit 0)
        out pins, 1         ; Instantly applies Bit 0 directly to the CLOCK pin
        
        ; 3. Extract PB1 (Bit 1)
        out y, 1        
        jmp !y check_pb4    ; If PB1 is 0, check PB4
        jmp drive_1         ; If PB1 is 1, assert DATA immediately
        
    check_pb4:
        ; 4. Extract PB4 (Bit 2)
        out y, 1        
        
        ; 5. Evaluate XOR: (PB4 ^ ATN)
        jmp pin atn_1   
        
    atn_0:
        jmp !y drive_0
        jmp drive_1
        
    atn_1:
        jmp !y drive_1
        jmp drive_0
        
    drive_0:
        set pins, 0         ; Release DATA
        jmp loop
        
    drive_1:
        set pins, 1         ; Assert DATA
        jmp loop
        "#
    )
    .program;

    let iec_loaded = pio1.common.load_program(&iec_prg);
    let data_tx_pin = pio1.common.make_pio_pin(unsafe { PinIecDataTx::steal() });
    let clk_tx_pin = pio1.common.make_pio_pin(unsafe { PinIecClkTx::steal() });
    let atn_pin = pio1.common.make_pio_pin(unsafe { PinIecAtnRx::steal() });

    let mut iec_cfg = embassy_rp::pio::Config::default();
    iec_cfg.use_program(&iec_loaded, &[]);

    // `set_pins` controls DATA_TX
    iec_cfg.set_set_pins(&[&data_tx_pin]);

    // `out_pins` controls CLK_TX
    iec_cfg.set_out_pins(&[&clk_tx_pin]);

    iec_cfg.set_in_pins(&[&atn_pin]);
    iec_cfg.set_jmp_pin(&atn_pin);

    // Shift right, auto-pull disabled, 32-bit threshold
    iec_cfg.shift_out.direction = embassy_rp::pio::ShiftDirection::Right;
    iec_cfg.shift_out.auto_fill = false;

    pio1.sm0.set_config(&iec_cfg);

    // Configure pin directions for PIO
    pio1.sm0.set_pin_dirs(
        embassy_rp::pio::Direction::Out,
        &[&data_tx_pin, &clk_tx_pin],
    );
    pio1.sm0
        .set_pin_dirs(embassy_rp::pio::Direction::In, &[&atn_pin]);

    pio1.sm0.set_enable(true);

    core::mem::forget(data_tx_pin);
    core::mem::forget(clk_tx_pin);
    core::mem::forget(atn_pin);
    core::mem::forget(iec_loaded);
    core::mem::forget(pio1); // DO NOT DROP PIO1, otherwise Drop will disable the state machine!
}

use pico1541_common::interface::TrackHandoff;
use core::sync::atomic::{AtomicU8};
use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

static TRACK_REQUEST: AtomicU8 = AtomicU8::new(34);
static TRACK_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub struct RpTrackHandoff;

impl RpTrackHandoff {
    pub const fn new() -> Self {
        Self
    }
}

impl TrackHandoff for RpTrackHandoff {
    fn notify_track_change(&mut self, half_track: u8) {
        TRACK_REQUEST.store(half_track, Ordering::Relaxed);
        TRACK_SIGNAL.signal(());
        
        embassy_rp::pac::TIMER0
            .intf()
            .write(|w| w.set_alarm(3, true));
    }

    fn current_track(&self) -> u8 {
        TRACK_REQUEST.load(Ordering::Relaxed)
    }

    async fn wait_for_change(&mut self) -> u8 {
        TRACK_SIGNAL.wait().await;
        self.current_track()
    }
}
