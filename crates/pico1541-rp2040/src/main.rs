#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use rp2040_boot2 as _;
use static_cell::StaticCell;

use pico1541_emulator::Emulator;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_manager::{Manager, EVENT_CHANNEL, StorageDriver, DisplayDriver, InputDriver, Error};

// --- Hardware Pins Definition ---

struct IecPins {
    data_rx: Input<'static>,
    clk_rx: Input<'static>,
    atn_rx: Input<'static>,
    data_tx: Output<'static>,
    clk_tx: Output<'static>,
}

// --- Driver Implementations ---

struct RpStorage;
impl StorageDriver for RpStorage {
    async fn read_block(&mut self, _track: u8, _sector: u8, _buf: &mut [u8]) -> Result<(), Error> {
        // TODO: Implement SD Card read using GPIO 11-15
        Ok(())
    }
    async fn write_block(&mut self, _track: u8, _sector: u8, _buf: &[u8]) -> Result<(), Error> {
        // TODO: Implement SD Card write using GPIO 11-15
        Ok(())
    }
}

struct RpDisplay {
    // TODO: Add I2C handle for GPIO 4, 5
}
impl DisplayDriver for RpDisplay {
    async fn clear(&mut self) {
        // TODO: Implement OLED clear
    }
    async fn print(&mut self, _line: u8, _text: &str) {
        // TODO: Implement OLED print
    }
}

struct RpInput {
    // TODO: Add GPIOs for Keyboard Matrix 6, 7, 8, 9
}
impl InputDriver for RpInput {
    async fn wait_for_button(&mut self) -> u8 {
        // TODO: Implement button wait using Matrix
        Timer::after(Duration::from_secs(1)).await;
        0
    }
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
        // Inverter: Bus Asserted (LOW) -> GPIO HIGH
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
        // Inverter: GPIO HIGH -> Bus Asserted (LOW)
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
        if let Err(_) = EVENT_CHANNEL.try_send(event) {
            warn!("Event channel full, event dropped");
        }
    }
}

// --- Multicore Setup ---

static CORE1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static EXECUTOR_CORE1: StaticCell<embassy_executor::Executor> = StaticCell::new();

#[embassy_executor::task]
async fn manager_task() {
    info!("Manager Task Started on Core 1");
    
    let storage = RpStorage;
    let display = RpDisplay {};
    let input = RpInput {};
    
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

    // Initialize IEC Pins
    let iec_pins = IecPins {
        data_rx: Input::new(p.PIN_16, Pull::None),
        clk_rx: Input::new(p.PIN_17, Pull::None),
        atn_rx: Input::new(p.PIN_18, Pull::None),
        data_tx: Output::new(p.PIN_19, Level::Low),
        clk_tx: Output::new(p.PIN_20, Level::Low),
    };

    // Initialize UI Pins (Skeletons)
    let _piezo = Output::new(p.PIN_2, Level::Low);
    let _pwr_led_pos = Output::new(p.PIN_22, Level::Low);
    let _pwr_led_neg = Output::new(p.PIN_3, Level::Low);
    // RGB Status LED
    let _red = Output::new(p.PIN_26, Level::High);
    let _green = Output::new(p.PIN_27, Level::High);
    let _blue = Output::new(p.PIN_28, Level::High);

    // Spawn Core 1 (Manager)
    let stack = CORE1_STACK.init(Stack::new());
    spawn_core1(p.CORE1, stack, move || {
        core1_run();
    });

    info!("Core 0: Entering Emulator Loop (1 MHz)");

    let mut bridge = HardwareBridge::new(iec_pins);
    
    // TODO: Load real 1541 ROM
    let rom = [0u8; 16384]; 
    let mut drive = Emulator::new(rom);
    drive.reset();

    // Use TIMER peripheral directly via PAC or via embassy-rp if available.
    // embassy_rp::pac is available with unstable-pac feature.
    let timer = embassy_rp::pac::TIMER;

    let mut last_tick = timer.timelr().read();

    loop {
        // Wait for next microsecond (1 MHz)
        // Note: RP2040 TIMER is 1 MHz by default.
        while timer.timelr().read() == last_tick {
            cortex_m::asm::nop();
        }
        last_tick = timer.timelr().read();

        // One 6502 cycle = 1 microsecond
        drive.tick(&mut bridge);
    }
}
