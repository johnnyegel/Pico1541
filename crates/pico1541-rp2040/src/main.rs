#![no_std]
#![no_main]

mod driver;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull, Pin};
use embassy_rp::i2c::{I2c, Config, InterruptHandler};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::I2C0;
use embassy_rp::bind_interrupts;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use panic_probe as _;
use rp2040_boot2 as _;
use static_cell::StaticCell;

use pico1541_emulator::Emulator;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_manager::EVENT_CHANNEL;
use pico1541_manager::tasks::{DisplayTask, ActivityTask, KeyboardTask};

use crate::driver::storage::RpStorage;
use crate::driver::display::{RpDisplay, DisplayWrapper};
use crate::driver::input::RpInput;
use crate::driver::leds::{StatusLeds, ActivityLed};
use crate::driver::speaker::RpSpeaker;
use crate::driver::indicator::RpIndicator;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

// --- Hardware Pins Definition ---

struct IecPins {
    data_rx: Input<'static>,
    clk_rx: Input<'static>,
    atn_rx: Input<'static>,
    data_tx: Output<'static>,
    clk_tx: Output<'static>,
}

// --- Hardware Bridge (Core 0) ---

struct HardwareBridge {
    track: ActiveTrack,
    pins: IecPins,
}

impl HardwareBridge {
    fn new(pins: IecPins) -> Self {
        Self {
            track: ActiveTrack::new(),
            pins,
        }
    }
}

impl IecBus for HardwareBridge {
    #[inline(always)]
    fn read_data(&self) -> bool {
        self.pins.data_rx.is_high()
    }
    #[inline(always)]
    fn read_clk(&self) -> bool {
        self.pins.clk_rx.is_high()
    }
    #[inline(always)]
    fn read_atn(&self) -> bool {
        self.pins.atn_rx.is_high()
    }
    #[inline(always)]
    fn drive_data(&mut self, assert: bool) {
        if assert {
            self.pins.data_tx.set_high();
        } else {
            self.pins.data_tx.set_low();
        }
    }
    #[inline(always)]
    fn drive_clk(&mut self, assert: bool) {
        if assert {
            self.pins.clk_tx.set_high();
        } else {
            self.pins.clk_tx.set_low();
        }
    }
}

impl HostInterface for HardwareBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        &mut self.track
    }

    fn send_event(&mut self, event: DriveEvent) {
        // Forward all events to Manager core via Channel
        if let Err(_) = EVENT_CHANNEL.try_send(event) {
            // Channel full
        }
    }
}

// --- Embassy Task Wrappers ---

#[embassy_executor::task]
async fn display_task(mut task: DisplayTask<DisplayWrapper<NoopRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn activity_task(mut task: ActivityTask<RpIndicator>) {
    task.run().await;
}

#[embassy_executor::task]
async fn keyboard_task(mut task: KeyboardTask<RpInput>) {
    task.run().await;
}

// --- Multicore Setup ---

static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static EXECUTOR_CORE1: StaticCell<embassy_executor::Executor> = StaticCell::new();
static DISPLAY: StaticCell<Mutex<NoopRawMutex, RpDisplay>> = StaticCell::new();

fn core1_run(display: &'static Mutex<NoopRawMutex, RpDisplay>, indicator: RpIndicator) -> ! {
    let executor = EXECUTOR_CORE1.init(embassy_executor::Executor::new());
    executor.run(|spawner| {
        let input = RpInput::new();
        let display_wrapper = DisplayWrapper(display);
        
        // Initialize Manager tasks
        let disp_task = DisplayTask::new(display_wrapper);
        let act_task = ActivityTask::new(indicator);
        let key_task = KeyboardTask::new(input);

        // Spawn Embassy tasks
        spawner.spawn(display_task(disp_task)).unwrap();
        spawner.spawn(activity_task(act_task)).unwrap();
        spawner.spawn(keyboard_task(key_task)).unwrap();
    });
}

// --- Core 0: Emulator ---

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Core 0: Starting Pico1541");

    // Initialize Status RGB LED
    let mut status_leds = StatusLeds::new(
        p.PIN_26.degrade(),
        p.PIN_27.degrade(),
        p.PIN_28.degrade(),
    );
    status_leds.set_color(false, true, false);

    // Initialize Activity LED
    let activity_led = ActivityLed::new(
        p.PIN_22.degrade(),
        p.PIN_3.degrade(),
    );

    // Initialize Speaker
    let speaker = RpSpeaker::new(p.PIN_2.degrade());
    
    // Combine into Indicator driver for Manager
    let indicator = RpIndicator::new(activity_led, speaker);

    // Initialize I2C for Display (GPIO 4, 5)
    let i2c = I2c::new_async(p.I2C0, p.PIN_5, p.PIN_4, Irqs, Config::default());
    let rp_display = RpDisplay::new(i2c);
    
    let display_ref = DISPLAY.init(Mutex::new(rp_display));
    
    // Initialize display hardware (async)
    {
        let mut d = display_ref.lock().await;
        d.init().await;
    }

    // Initialize IEC Pins
    let iec_pins = IecPins {
        data_rx: Input::new(p.PIN_16, Pull::None),
        clk_rx: Input::new(p.PIN_17, Pull::None),
        atn_rx: Input::new(p.PIN_18, Pull::None),
        data_tx: Output::new(p.PIN_19, Level::Low),
        clk_tx: Output::new(p.PIN_20, Level::Low),
    };

    // Spawn Core 1 (Manager)
    let stack = CORE1_STACK.init(Stack::new());
    spawn_core1(p.CORE1, stack, move || {
        core1_run(display_ref, indicator);
    });

    info!("Core 0: Entering Emulator Loop (1 MHz)");

    let mut bridge = HardwareBridge::new(iec_pins);
    
    // TODO: Load real 1541 ROM
    let rom = [0u8; 16384]; 
    let mut drive = Emulator::new(rom);
    drive.reset();

    // Use TIMER peripheral directly via PAC
    let timer = embassy_rp::pac::TIMER;

    let mut last_tick = timer.timelr().read();

    loop {
        // Wait for next microsecond (1 MHz)
        while timer.timelr().read() == last_tick {
            cortex_m::asm::nop();
        }
        last_tick = timer.timelr().read();

        // One 6502 cycle = 1 microsecond
        drive.tick(&mut bridge);
    }
}
