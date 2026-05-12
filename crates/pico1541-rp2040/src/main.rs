#![no_std]
#![no_main]

mod driver;

use defmt::*;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, I2c, InterruptHandler};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;

use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler, SnooperConfig};
use pico1541_common::formats::{AnyDiskImage, VirtualDiskImage};
use pico1541_common::storage::{DirectoryEntry, StaticDirectorySource, MemoryStorage};
use pico1541_emulator::Emulator;
use pico1541_manager::tasks::{DisplayTask, DriveActivityTask, SnooperTask, VirtualDiskTask};
use panic_probe as _;

use crate::driver::display::{DisplayWrapper, OledDisplay};
use crate::driver::indicator::ActivityIndicator;
use crate::driver::leds::ActivityLed;
use crate::driver::speaker::PiezoSpeaker;

use static_cell::StaticCell;
use portable_atomic::Ordering;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(8);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("Pico1541 Starting...");

    // --- Hardware Drivers (Core 1 - Manager) ---

    // 1. OLED Display (I2C)
    let sda = p.PIN_4;
    let scl = p.PIN_5;
    let i2c = I2c::new_async(p.I2C0, scl, sda, Irqs, Config::default());

    static OLED: StaticCell<Mutex<NoopRawMutex, OledDisplay>> = StaticCell::new();
    let mut oled_obj = OledDisplay::new(i2c);
    oled_obj.init().await;
    let oled_mutex = OLED.init(Mutex::new(oled_obj));
    let display_task = DisplayTask::new(DisplayWrapper(oled_mutex));

    // 2. Indicators (Piezo + LEDs)
    let activity_led = ActivityLed::new(p.PIN_22, p.PIN_3);
    let piezo = PiezoSpeaker::new(p.PIN_2);
    let indicator = ActivityIndicator::new(activity_led, piezo);
    let activity_task = DriveActivityTask::new(indicator);

    // 3. Snooper Task
    let snooper_task = SnooperTask::new(8);

    // 4. Storage (SD Card - SPI)
    // For now, use a virtual disk with static content
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
    
    // Sample file data for "GAME"
    static GAME_DATA: &[u8] = include_bytes!("../../../crates/pico1541-emulator/tests/data/game.prg");
    vdisk.file_storage = Some(MemoryStorage::new(GAME_DATA));
    
    static DISK_IMAGE: StaticCell<AnyDiskImage<MemoryStorage<'static>, StaticDirectorySource<'static>>> = StaticCell::new();
    let disk_image = DISK_IMAGE.init(AnyDiskImage::VirtualDisk(vdisk));

    // 5. Keyboard (Matrix)
    // TODO: Initialize keyboard driver

    // Shared ActiveTrack buffer
    static ACTIVE_TRACK: StaticCell<ActiveTrack> = StaticCell::new();
    let track_ptr = ACTIVE_TRACK.init(ActiveTrack::new()) as *mut ActiveTrack;

    // Safety: Core 0 (Emulator) and Core 1 (Manager) share the ActiveTrack buffer.
    // Core 0 reads/writes it in a cycle-exact loop, while Core 1 updates it during loads.
    let track_for_task = unsafe { &mut *track_ptr };
    let vdisk_task = VirtualDiskTask::new(disk_image, track_for_task);

    // Spawn Manager Tasks
    spawner.spawn(display_run(display_task).unwrap());
    spawner.spawn(activity_run(activity_task).unwrap());
    spawner.spawn(snooper_run(snooper_task).unwrap());
    spawner.spawn(vdisk_run(vdisk_task).unwrap());

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

    // 1541-II ROM (Stock)
    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let drive = Emulator::new(*rom, 8);

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

// --- Hardware Bridge (Core 0) ---

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
        // Combinatorial: Drive's output OR Snooper's hijack output
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
        if let Err(_) = pico1541_manager::EVENT_CHANNEL.try_send(event) {
            // Channel full
        }
    }

    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(Ordering::Relaxed)
    }

    fn pop_discard_request(&mut self) -> bool {
        let req = self.config.discard_request.swap(false, Ordering::Relaxed);
        req
    }

    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, Ordering::Relaxed);
        self.config.talk_addr.store(talk, Ordering::Relaxed);
    }

    fn sync(&mut self) {
        let host = self.iec_host();
        let config = self.config;
        
        // Pass a mutable reference to self.handler via the closure's environment
        let handler = &mut self.handler;
        
        self.snooper.update(host, |event| {
            // Forward events to Core 1
            if let Err(_) = pico1541_manager::EVENT_CHANNEL.try_send(event) { }
            
            // Handle for hijacking on Core 0
            handler.handle(event, |e| {
                 if let Err(_) = pico1541_manager::EVENT_CHANNEL.try_send(e) { }
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

// --- Embassy Task Wrappers ---

#[embassy_executor::task]
async fn display_run(mut task: DisplayTask<DisplayWrapper<NoopRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn activity_run(mut task: DriveActivityTask<ActivityIndicator>) {
    task.run().await;
}

#[embassy_executor::task]
async fn snooper_run(mut task: SnooperTask) {
    task.run().await;
}

#[embassy_executor::task]
async fn vdisk_run(mut task: VirtualDiskTask<'static, MemoryStorage<'static>, StaticDirectorySource<'static>>) {
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
