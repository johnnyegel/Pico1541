#![no_std]
#![no_main]

mod driver;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull, Pin};
use embassy_rp::multicore::{spawn_core1, Stack};
use panic_probe as _;
use rp2040_boot2 as _;
use static_cell::StaticCell;

use pico1541_emulator::Emulator;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_manager::{Manager, EVENT_CHANNEL};

use crate::driver::storage::RpStorage;
use crate::driver::display::RpDisplay;
use crate::driver::input::RpInput;
use crate::driver::leds::{StatusLeds, ActivityLed};
use crate::driver::speaker::RpSpeaker;

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
    activity_led: ActivityLed,
    speaker: RpSpeaker,
}

impl HardwareBridge {
    fn new(pins: IecPins, activity_led: ActivityLed, speaker: RpSpeaker) -> Self {
        Self {
            track: ActiveTrack::new(),
            pins,
            activity_led,
            speaker,
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
        match event {
            DriveEvent::LedChanged(active) => {
                self.activity_led.set_state(active);
            }
            DriveEvent::HeadStepped { .. } => {
                self.speaker.beep();
            }
            _ => {}
        }
        if let Err(_) = EVENT_CHANNEL.try_send(event) {
            // Channel full, ignore for now
        }
    }
}

// --- Multicore Setup ---

static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static EXECUTOR_CORE1: StaticCell<embassy_executor::Executor> = StaticCell::new();

#[embassy_executor::task]
async fn manager_task() {
    info!("Manager Task Started on Core 1");
    
    let storage = RpStorage::new();
    let display = RpDisplay::new();
    let input = RpInput::new();
    
    let mut manager = Manager::new(storage, display, input);
    manager.run().await;
}

fn core1_run() -> ! {
    let executor = EXECUTOR_CORE1.init(embassy_executor::Executor::new());
    executor.run(|spawner| {
        spawner.spawn(manager_task()).unwrap();
    });
}

// --- Core 0: Emulator ---

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Core 0: Starting Pico1541");

    // Initialize Status RGB LED (Core 0/1 can use this for firmware status)
    let mut status_leds = StatusLeds::new(
        p.PIN_26.degrade(),
        p.PIN_27.degrade(),
        p.PIN_28.degrade(),
    );
    // Set to GREEN to indicate boot success
    status_leds.set_color(false, true, false);

    // Initialize Activity LED (Drive activity)
    let activity_led = ActivityLed::new(
        p.PIN_22.degrade(),
        p.PIN_3.degrade(),
    );

    // Initialize Speaker
    let speaker = RpSpeaker::new(p.PIN_2.degrade());

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
        core1_run();
    });

    info!("Core 0: Entering Emulator Loop (1 MHz)");

    let mut bridge = HardwareBridge::new(iec_pins, activity_led, speaker);
    
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
