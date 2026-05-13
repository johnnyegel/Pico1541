#![no_std]
#![no_main]

mod driver;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, InterruptHandler};
use embassy_rp::peripherals::{I2C0, SPI1, PIN_25};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::Peri;
use embassy_rp::multicore::Stack;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_hal_bus::spi::ExclusiveDevice;

use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler, SnooperConfig};
use pico1541_common::formats::{AnyDiskImage, VirtualDiskImage};
use pico1541_common::storage::{DirectoryEntry, StaticDirectorySource, MemoryStorage, RuntimeStorage};
use pico1541_emulator::Emulator;
use pico1541_manager::tasks::{
    DisplayTask, DriveActivityTask, KeyboardTask, MainMenuTask, SnooperTask, VirtualDiskTask,
};
use panic_probe as _;

use crate::driver::display::{DisplayWrapper, OledDisplay};
use crate::driver::indicator::{ActivityIndicator, IndicatorWrapper};
use crate::driver::input::KeyboardInput;
use crate::driver::leds::{ActivityLed, StatusLeds, StatusLedsWrapper};
use crate::driver::speaker::PiezoSpeaker;
use crate::driver::storage::{SdFileHandle, SdStorage, StorageWrapper};

use static_cell::StaticCell;
use portable_atomic::Ordering;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(8);

// Core 1 Stack
static CORE1_STACK: StaticCell<Stack<4096>> = StaticCell::new();

type MySpi = Spi<'static, SPI1, embassy_rp::spi::Blocking>;
type MyCs = Output<'static>;
type MyDelay = embassy_time::Delay;
type MySpiDev = ExclusiveDevice<MySpi, MyCs, MyDelay>;
type MyFile = SdFileHandle<MySpiDev, DummyPin, MyDelay>;
type MyStorage = RuntimeStorage<'static, MyFile>;
type MyDir = StaticDirectorySource<'static>;
type MyDisk = AnyDiskImage<MyStorage, MyDir>;
// In embedded-sdmmc 0.7, SdCard takes SPI, CS, DELAYER.
// If SPI is SpiDevice, CS is STILL required? No, usually not.
// Wait, I will use a dummy pin for CS if SdCard insists on it.
struct DummyPin;
impl embedded_hal::digital::ErrorType for DummyPin { type Error = core::convert::Infallible; }
impl embedded_hal::digital::OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), Self::Error> { Ok(()) }
    fn set_high(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Pico1541 Starting...");

    let stack = CORE1_STACK.init(Stack::new());
    // --- Hardware Drivers (Core 1 - Manager) ---

    // 1. OLED Display (I2C)
    let sda = p.PIN_4;
    let scl = p.PIN_5;
    let i2c = embassy_rp::i2c::I2c::new_async(p.I2C0, scl, sda, Irqs, Config::default());

    static OLED: StaticCell<Mutex<CriticalSectionRawMutex, OledDisplay>> = StaticCell::new();
    let mut oled_obj = OledDisplay::new(i2c);
    oled_obj.init().await;
    let oled_mutex = OLED.init(Mutex::new(oled_obj));

    // 2. Indicators (Piezo + LEDs)
    let activity_led = ActivityLed::new(p.PIN_22, p.PIN_3);
    let piezo = PiezoSpeaker::new(p.PIN_2);
    let indicator = ActivityIndicator::new(activity_led, piezo);

    static INDICATOR: StaticCell<Mutex<CriticalSectionRawMutex, ActivityIndicator>> =
        StaticCell::new();
    let indicator_mutex = INDICATOR.init(Mutex::new(indicator));
    let activity_task = DriveActivityTask::new(IndicatorWrapper(indicator_mutex));

    // 3. RGB Status LEDs (Used in Hardware Test via Main Menu)
    let status_leds_obj = StatusLeds::new(p.PIN_26, p.PIN_27, p.PIN_28);
    static STATUS_LEDS: StaticCell<Mutex<CriticalSectionRawMutex, StatusLeds>> = StaticCell::new();
    let status_leds_mutex = STATUS_LEDS.init(Mutex::new(status_leds_obj));

    // 4. SD Card (SPI1)
    let mosi = p.PIN_15;
    let miso = p.PIN_12;
    let clk = p.PIN_14;
    let cs = Output::new(p.PIN_13, Level::High);
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = 400_000; // Start slow for SD init
    let spi = Spi::new_blocking(p.SPI1, clk, mosi, miso, spi_config);
    let spi_dev = ExclusiveDevice::new(spi, cs, embassy_time::Delay).unwrap();

    static SD_STORAGE: StaticCell<Mutex<CriticalSectionRawMutex, SdStorage<MySpiDev, DummyPin, MyDelay>>> = StaticCell::new();
    let sd_obj = SdStorage::new(spi_dev, DummyPin, embassy_time::Delay);
    let sd_mutex = SD_STORAGE.init(Mutex::new(sd_obj));

    // 5. Keyboard Matrix
    let keyboard = KeyboardInput::new(p.PIN_6, p.PIN_7, p.PIN_8, p.PIN_9);
    let keyboard_task = KeyboardTask::new(keyboard);

    // 6. Display Task
    let display_task = DisplayTask::new(DisplayWrapper(oled_mutex));

    // 7. Snooper Task
    let snooper_task = SnooperTask::new(8);

    // 8. Storage (Virtual Disk for fallback)
    static DIR_ENTRIES: [DirectoryEntry; 1] = [
        DirectoryEntry {
            file_type: 0x82,
            filename: [b'G', b'A', b'M', b'E', 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0, 0xA0],
            first_track: 1,
            first_sector: 0,
            block_size: 10,
        },
    ];
    let dir_source = StaticDirectorySource::new(b"VIRTUAL DISK", &DIR_ENTRIES);
    let mut vdisk = VirtualDiskImage::new(dir_source);
    
    static GAME_DATA: &[u8] = include_bytes!("../../../crates/pico1541-emulator/tests/data/game.prg");
    vdisk.file_storage = Some(RuntimeStorage::Memory(MemoryStorage::new(GAME_DATA)));
    
    static DISK_IMAGE: StaticCell<Mutex<CriticalSectionRawMutex, MyDisk>> = StaticCell::new();
    let disk_image_mutex = DISK_IMAGE.init(Mutex::new(AnyDiskImage::VirtualDisk(vdisk)));

    // Shared ActiveTrack buffer
    static ACTIVE_TRACK: StaticCell<ActiveTrack> = StaticCell::new();
    let track_ptr = ACTIVE_TRACK.init(ActiveTrack::new()) as *mut ActiveTrack;

    let track_for_task = unsafe { &mut *track_ptr };
    let vdisk_task = VirtualDiskTask::new(disk_image_mutex, track_for_task);

    // --- Spawn Core 1 ---
    let menu_task = MainMenuTask::new(
        DisplayWrapper(oled_mutex),
        StatusLedsWrapper(status_leds_mutex),
        IndicatorWrapper(indicator_mutex),
        StorageWrapper(sd_mutex),
        disk_image_mutex,
    );
    let pin25 = p.PIN_25;

    embassy_rp::multicore::spawn_core1(p.CORE1, stack, move || {
        core1_main(
            menu_task,
            activity_task,
            keyboard_task,
            display_task,
            snooper_task,
            vdisk_task,
            pin25,
        );
    });

    // IEC Pins
    let pins = IecPins {
        data_rx: Input::new(p.PIN_16, Pull::None),
        clk_rx: Input::new(p.PIN_17, Pull::None),
        atn_rx: Input::new(p.PIN_18, Pull::None),
        data_tx: Output::new(p.PIN_19, Level::Low),
        clk_tx: Output::new(p.PIN_20, Level::Low),
    };

    let track_for_bridge = unsafe { &mut *track_ptr };
    let bridge = HardwareBridge::new(pins, track_for_bridge, &SNOOPER_CONFIG);

    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let drive = Emulator::new(*rom, 8);

    emulator_loop(drive, bridge).await;
}

struct IecPins {
    data_rx: Input<'static>,
    clk_rx: Input<'static>,
    atn_rx: Input<'static>,
    data_tx: Output<'static>,
    clk_tx: Output<'static>,
}

struct HardwareBridge {
    track: &'static mut ActiveTrack,
    pins: IecPins,
    snooper: IecSnooper,
    handler: IecSnoopByteHandler,
    config: &'static SnooperConfig,
}

impl HardwareBridge {
    fn new(pins: IecPins, track: &'static mut ActiveTrack, config: &'static SnooperConfig) -> Self {
        Self {
            track,
            pins,
            snooper: IecSnooper::new(config.device_id),
            handler: IecSnoopByteHandler::new(),
            config,
        }
    }
}

impl IecBus for HardwareBridge {
    #[inline(always)]
    fn iec_host(&self) -> IecBusState {
        let mut state = IecBusState::default();
        state.set_data(self.pins.data_rx.is_high());
        state.set_clk(self.pins.clk_rx.is_high());
        state.set_atn(self.pins.atn_rx.is_high());
        state
    }

    #[inline(always)]
    fn iec_drive(&mut self, state: IecBusState) {
        let should_drive_data = self.config.should_drive_data.load(Ordering::Relaxed);
        
        if state.get_data() || should_drive_data {
            self.pins.data_tx.set_high();
        } else {
            self.pins.data_tx.set_low();
        }

        if state.get_clk() {
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
        if pico1541_manager::EVENT_CHANNEL.try_send(event).is_err() {
            // Channel full
        }
    }

    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(Ordering::Relaxed)
    }

    fn pop_discard_request(&mut self) -> bool {
        self.config.discard_request.swap(false, Ordering::Relaxed)
    }

    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, Ordering::Relaxed);
        self.config.talk_addr.store(talk, Ordering::Relaxed);
    }

    fn sync(&mut self) {
        let host = self.iec_host();
        let config = self.config;
        let handler = &mut self.handler;
        
        self.snooper.update(host, |event| {
            let _ = pico1541_manager::EVENT_CHANNEL.try_send(event);
            handler.handle(event, |e| {
                 let _ = pico1541_manager::EVENT_CHANNEL.try_send(e);
            });
            if handler.is_hijacked() {
                config.hijack_active.store(true, Ordering::Relaxed);
                config.should_drive_data.store(true, Ordering::Relaxed);
            }
            if handler.should_discard() {
                config.discard_request.store(true, Ordering::Relaxed);
                config.hijack_active.store(false, Ordering::Relaxed);
                config.should_drive_data.store(false, Ordering::Relaxed);
            }
        });
    }
}

#[embassy_executor::task]
async fn activity_run(mut task: DriveActivityTask<IndicatorWrapper<CriticalSectionRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn snooper_run(mut task: SnooperTask) {
    task.run().await;
}

#[embassy_executor::task]
async fn vdisk_run(mut task: VirtualDiskTask<'static, MyStorage, MyDir>) {
    task.run().await;
}

#[embassy_executor::task]
async fn display_run(mut task: DisplayTask<DisplayWrapper<CriticalSectionRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn keyboard_run(mut task: KeyboardTask<KeyboardInput>) {
    task.run().await;
}

#[embassy_executor::task]
async fn menu_run(
    mut task: MainMenuTask<
        DisplayWrapper<CriticalSectionRawMutex>,
        StatusLedsWrapper<CriticalSectionRawMutex>,
        IndicatorWrapper<CriticalSectionRawMutex>,
        StorageWrapper<MySpiDev, DummyPin, MyDelay>,
        MyDisk,
    >,
) {
    task.run().await;
}

#[embassy_executor::task]
async fn blinky(pin: Peri<'static, PIN_25>) {
    let mut led = Output::new(pin, Level::Low);
    info!("Heartbeat blinky task started!");
    loop {
        led.set_high();
        embassy_time::Timer::after_millis(100).await;
        led.set_low();
        embassy_time::Timer::after_millis(900).await;
    }
}

fn core1_main(
    menu_task: MainMenuTask<
        DisplayWrapper<CriticalSectionRawMutex>,
        StatusLedsWrapper<CriticalSectionRawMutex>,
        IndicatorWrapper<CriticalSectionRawMutex>,
        StorageWrapper<MySpiDev, DummyPin, MyDelay>,
        MyDisk,
    >,
    activity_task: DriveActivityTask<IndicatorWrapper<CriticalSectionRawMutex>>,
    keyboard_task: KeyboardTask<KeyboardInput>,
    display_task: DisplayTask<DisplayWrapper<CriticalSectionRawMutex>>,
    snooper_task: SnooperTask,
    vdisk_task: VirtualDiskTask<'static, MyStorage, MyDir>,
    pin25: Peri<'static, PIN_25>,
) -> ! {
    static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();
    let executor = EXECUTOR.init(embassy_executor::Executor::new());

    executor.run(|spawner| {
        spawner.spawn(menu_run(menu_task).unwrap());
        spawner.spawn(activity_run(activity_task).unwrap());
        spawner.spawn(keyboard_run(keyboard_task).unwrap());
        spawner.spawn(display_run(display_task).unwrap());
        spawner.spawn(snooper_run(snooper_task).unwrap());
        spawner.spawn(vdisk_run(vdisk_task).unwrap());
        spawner.spawn(blinky(pin25).unwrap());
    })
}

async fn emulator_loop(mut drive: Emulator<HardwareBridge>, mut bridge: HardwareBridge) -> ! {
    let timer = embassy_rp::pac::TIMER;
    let mut last_tick = timer.timelr().read();

    loop {
        while timer.timelr().read() == last_tick {
            cortex_m::asm::nop();
        }
        last_tick = timer.timelr().read();
        drive.tick(&mut bridge);
        bridge.sync();
    }
}
