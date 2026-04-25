#![no_std]
#![no_main]

mod driver;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull, Pin};
use embassy_rp::i2c::{I2c, Config, InterruptHandler};
use embassy_rp::peripherals::{I2C0, PIO0, SPI0};
use embassy_rp::spi;
use embassy_rp::timer::Timer;
use embassy_rp::bind_interrupts;
use embassy_rp::pio::Pio;
use embassy_rp::clocks::Rosc;
use embassy_rp::Peripheral;

use pico1541_common::interface::{ActiveTrack, DriveEvent};
use pico1541_emulator::Emulator;
use pico1541_emulator::interface::{IecBus, HostInterface};
use pico1541_manager::tasks::{DisplayTask, DriveActivityTask, KeyboardTask};

use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Pico1541 Starting...");

    // --- Hardware Drivers (Core 1 - Manager) ---
    
    // 1. OLED Display (I2C)
    let sda = p.PIN_4;
    let scl = p.PIN_5;
    let i2c = I2c::new(p.I2C0, scl, sda, Irqs, Config::default());
    let mut display_task = DisplayTask::new(i2c);

    // 2. Indicators (Piezo + LEDs)
    let piezo = Output::new(p.PIN_2, Level::Low);
    let activity_led = Output::new(p.PIN_22, Level::Low);
    let mut activity_task = DriveActivityTask::new(activity_led, piezo);

    // 3. Storage (SD Card - SPI)
    // TODO: Initialize SD card driver
    
    // 4. Keyboard (Matrix)
    // TODO: Initialize keyboard driver
    
    // Spawn Manager Tasks
    spawner.spawn(display_run(display_task)).unwrap();
    spawner.spawn(activity_run(activity_task)).unwrap();

    // --- Emulator Setup (Core 0 - 1541-II) ---

    // Shared ActiveTrack buffer
    static ACTIVE_TRACK: StaticCell<ActiveTrack> = StaticCell::new();
    let track = ACTIVE_TRACK.init(ActiveTrack::new());

    // IEC Pins
    let pins = IecPins {
        data_rx: Input::new(p.PIN_16, Pull::None),
        clk_rx: Input::new(p.PIN_17, Pull::None),
        atn_rx: Input::new(p.PIN_18, Pull::None),
        data_tx: Output::new(p.PIN_19, Level::Low),
        clk_tx: Output::new(p.PIN_20, Level::Low),
    };

    let bridge = HardwareBridge::new(pins, track, 8);

    // 1541-II ROM (Stock)
    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let mut drive = Emulator::new(*rom, 8);

    // Run Emulator Loop (Cycle-Exact 1 MHz)
    emulator_loop(drive, bridge).await;
}

// --- IEC Pin Wrapper ---

struct IecPins {
    data_rx: Input<'static>,
    clk_rx: Input<'static>,
    atn_rx: Input<'static>,
    data_tx: Output<'static>,
    clk_tx: Output<'static>,
}

use pico1541_emulator::snooper::IecSnooper;

// --- Hardware Bridge (Core 0) ---

struct HardwareBridge {
    track: &'static mut ActiveTrack,
    pins: IecPins,
    snooper: IecSnooper,
}

impl HardwareBridge {
    fn new(pins: IecPins, track: &'static mut ActiveTrack, device_id: u8) -> Self {
        Self {
            track,
            pins,
            snooper: IecSnooper::new(device_id),
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
        // Combinatorial: Drive's output OR Snooper's hijack output
        if assert || self.snooper.should_drive_data() {
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
        self.track
    }

    fn send_event(&mut self, event: DriveEvent) {
        if let Err(_) = pico1541_manager::EVENT_CHANNEL.try_send(event) {
            // Channel full
        }
    }

    fn is_hijacked(&self) -> bool {
        self.snooper.hijack_active
    }

    fn pop_discard_request(&mut self) -> bool {
        let req = self.snooper.discard_request;
        self.snooper.discard_request = false;
        req
    }

    fn sync(&mut self) {
        let atn = self.pins.atn_rx.is_high();
        let clk = self.pins.clk_rx.is_high();
        let data = self.pins.data_rx.is_high();

        // Passive snooping only
        if let Some(cmd_data) = self.snooper.tick(atn, clk, data) {
            let len = self.snooper.buffer_len() as u8;
            self.send_event(DriveEvent::IecCommand {
                data: cmd_data,
                len,
            });
        }
    }
}

// --- Embassy Task Wrappers ---

#[embassy_executor::task]
async fn display_run(mut task: DisplayTask<I2c<'static, I2C0>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn activity_run(mut task: DriveActivityTask<Output<'static>, Output<'static>>) {
    task.run().await;
}

// --- Emulator Loop ---

async fn emulator_loop(mut drive: Emulator<HardwareBridge>, mut bridge: HardwareBridge) -> ! {
    let timer = embassy_rp::pac::TIMER;
    let mut last_tick = timer.timelr().read();

    loop {
        while timer.timelr().read() == last_tick {
            cortex_m::asm::nop();
        }
        last_tick = timer.timelr().read();

        // One 6502 cycle = 1 microsecond
        drive.tick(&mut bridge);
        
        // Update snooper once per microsecond, outside the cycle core but in the main loop
        bridge.sync();
    }
}
