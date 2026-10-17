#![no_std]
#![no_main]

extern crate alloc;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

use embassy_rp::interrupt;
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;

use core::sync::atomic::Ordering;
use defmt::{error, info};
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, InterruptHandler};
use embassy_rp::multicore::Stack;
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, I2C0, PIO0, PIO1};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use pico1541_common::snooper::SnooperConfig;
use pico1541_common::DriveLocation;

use embassy_rp::block::{item_image_type_exe, item_vector_table, Architecture, Block, Security};

#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: Block<3> = Block::new([
    item_image_type_exe(Security::Secure, Architecture::Arm),
    item_vector_table(0x10000000)[0],
    item_vector_table(0x10000000)[1],
]);

pub const SYSTEM_CLOCK_FREQ: u32 = 250_000_000;
use pico1541_rp_common::emulation::bridge::RpTrackHandoff;
use pico1541_manager::tasks::{
    ActionTask, DisplayTask, KeyboardTask, MainMenuTask, TrackCacheTask, MountControllerTask,
};
use pico1541_manager::tasks::ui::UiContext;
use pico1541_manager::{ACTION_CHANNEL, CMD_CHANNEL, KEYBOARD_CHANNEL, SETTINGS_CHANNEL};

#[cfg(has_mklittlefs)]
#[link_section = ".fs"]
#[used]
static FS_IMAGE: [u8; 2048 * 1024] = *include_bytes!(concat!(env!("OUT_DIR"), "/fs.bin"));

// Local drivers
use pico1541_manager::driver::file_storage::{FatStorage, StorageWrapper};
use pico1541_rp_common::driver::blockdevsdcard::{SdBlockIo, SdCard};
use pico1541_rp_common::driver::buttons::DirectKeyboardInput;
use pico1541_rp_common::driver::display::{DisplayHardware, DisplayState, DisplayWrapper};
use pico1541_rp_common::driver::indicator::{ActivityIndicator, IndicatorWrapper};
use pico1541_rp_common::driver::leds::{StatusLeds, StatusLedsWrapper};
use pico1541_rp_common::driver::speaker::PiezoSpeaker;

// Tasks and manager components

use pico1541_common::track_buffer::{DualCoreTrackBuffer, ManagerHandle};

use static_cell::StaticCell;

#[link_section = ".shared_uninit"]
static DUAL_CORE_DATA: pico1541_common::track_buffer::DualCoreTrackData =
    pico1541_common::track_buffer::DualCoreTrackData::new();
#[link_section = ".shared_uninit"]
static DUAL_CORE_BUFFER: DualCoreTrackBuffer =
    DualCoreTrackBuffer::new(DUAL_CORE_DATA.buffers.get());

#[allow(unused_imports)]
use pico1541_rp_common::storage_manager::*;

use pico1541_rp_common::flash_storage::FlashStorage;
use pico1541_rp_common::littlefs_storage::LittleFsStorage;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<embassy_rp::peripherals::USB>;
    I2C0_IRQ => InterruptHandler<I2C0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO1>;
});

const DEFAULT_DRIVE_ID: u8 = 8;

static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(DEFAULT_DRIVE_ID);

static CURRENT_LOCATION: Mutex<CriticalSectionRawMutex, DriveLocation> =
    Mutex::new(DriveLocation::Root);
static MOTOR_STATE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

static EXECUTOR_HIGH: embassy_executor::InterruptExecutor =
    embassy_executor::InterruptExecutor::new();

#[embassy_rp::interrupt]
unsafe fn TIMER0_IRQ_3() {
    embassy_rp::pac::TIMER0
        .intf()
        .write(|w| w.set_alarm(3, false));
    EXECUTOR_HIGH.on_interrupt();
}

// Core 1 Stack
#[link_section = ".core1_stack"]
static mut CORE1_STACK: core::mem::MaybeUninit<Stack<1024>> = core::mem::MaybeUninit::uninit();

// ROM Buffer for dynamic loading
#[link_section = ".core1_uninit"]
static mut ROM_BUFFER: [u8; 16384] = [0; 16384];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    pico1541_emulator::init_core1_text();
    // Copy .ram_rodata from Flash to RAM
    unsafe {
        extern "C" {
            static mut __sram_rodata: u32;
            static mut __eram_rodata: u32;
            static __siram_rodata: u32;
        }
        let mut dest = core::ptr::addr_of_mut!(__sram_rodata);
        let end = core::ptr::addr_of_mut!(__eram_rodata);
        let mut src = core::ptr::addr_of!(__siram_rodata);
        while dest < end {
            core::ptr::write(dest, core::ptr::read(src));
            dest = dest.add(1);
            src = src.add(1);
        }
    }

    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 8192;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe {
            HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
        }
    }

    // Set VREG to 1.20V to safely support the 250MHz overclock
    // On RP2350, VREG control is different, skipping for now.

    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(SYSTEM_CLOCK_FREQ).unwrap();
    let p = embassy_rp::init(config);

    let usb_driver = embassy_rp::usb::Driver::new(p.USB, Irqs);
    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("Antigravity");
    usb_config.product = Some("Pico1541 USB Debug");
    usb_config.serial_number = Some("12345678");
    usb_config.max_power = 100;
    usb_config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static BOS_DESCRIPTOR: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static CONTROL_BUF: static_cell::StaticCell<[u8; 64]> = static_cell::StaticCell::new();

    let mut builder = embassy_usb::Builder::new(
        usb_driver,
        usb_config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    static CDC_STATE: static_cell::StaticCell<embassy_usb::class::cdc_acm::State> =
        static_cell::StaticCell::new();
    let mut _cdc = embassy_usb::class::cdc_acm::CdcAcmClass::new(
        &mut builder,
        CDC_STATE.init(embassy_usb::class::cdc_acm::State::new()),
        64,
    );
    let usb = builder.build();

    spawner.spawn(usb_task(usb).unwrap());

    info!(
        "Pico1541 Starting at {}MHz...",
        SYSTEM_CLOCK_FREQ / 1_000_000
    );

    // Initialize Flash for LittleFS
    // RP2350 Pico 2 has 4MB of flash. We configure 2MB for the internal LittleFS store.
    let flash_driver = embassy_rp::flash::Flash::<
        _,
        embassy_rp::flash::Blocking,
        { 4 * 1024 * 1024 },
    >::new_blocking(p.FLASH);
    let fs_storage_inst = FlashStorage::new(flash_driver);

    fn fs_config() -> littlefs_rust::Config {
        let mut config = littlefs_rust::Config::new(4096, 512); // 512 blocks * 4KB = 2048 KB (2 MB)
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 32;
        config
    }

    // Attempt to mount LittleFS, if it fails format it
    let mut fs_storage_inst = match littlefs_rust::Filesystem::mount(fs_storage_inst, fs_config()) {
        Ok(fs) => fs
            .unmount()
            .unwrap_or_else(|e| panic!("failed to unmount: {:?}", e)),
        Err((_, mut fs_storage)) => {
            info!("Formatting LittleFS...");
            if let Err(_e) = littlefs_rust::Filesystem::format(&mut fs_storage, &fs_config()) {
                error!("Failed to format LittleFS!");
            }
            fs_storage
        }
    };

    // Read Settings and Load ROM
    let mut drive_id = DEFAULT_DRIVE_ID;
    let mut rom_name = heapless::String::<32>::new();
    let _ = rom_name.push_str("-- Not available --");

    fs_storage_inst = match littlefs_rust::Filesystem::mount(fs_storage_inst, fs_config()) {
        Ok(fs) => {
            if let Ok(data) = fs.read_to_vec("/settings.dat") {
                if data.len() == 1 {
                    drive_id = data[0];
                }
            }

            let mut loaded_rom = false;

            // Try to load configured boot rom first
            if let Ok(file) = fs.open("/settings/boot_rom.txt", littlefs_rust::OpenFlags::READ) {
                let mut name_buf = [0u8; 32];
                if let Ok(n) = file.read(&mut name_buf) {
                    if let Ok(s) = core::str::from_utf8(&name_buf[..(n as usize)]) {
                        rom_name.clear();
                        let _ = rom_name.push_str(s);

                        let mut path = heapless::String::<64>::new();
                        let _ = path.push_str("/roms/");
                        let _ = path.push_str(s);
                        if let Ok(rom_file) = fs.open(path.as_str(), littlefs_rust::OpenFlags::READ)
                        {
                            if rom_file.size() == 16384 {
                                let buf = unsafe { &mut *core::ptr::addr_of_mut!(ROM_BUFFER) };
                                if let Ok(n) = rom_file.read(buf) {
                                    if n == 16384 {
                                        info!("Loaded configured ROM: {}", s);
                                        loaded_rom = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !loaded_rom {
                // Scan /roms/ for the first .bin file
                if let Ok(dir) = fs.read_dir("/roms") {
                    for entry in dir.flatten() {
                        let name_str = entry.name.as_str();
                        if name_str.ends_with(".bin")
                            || name_str.ends_with(".BIN")
                            || name_str.ends_with(".rom")
                            || name_str.ends_with(".ROM")
                        {
                            let mut path = heapless::String::<64>::new();
                            let _ = path.push_str("/roms/");
                            let _ = path.push_str(name_str);
                            if let Ok(file) = fs.open(path.as_str(), littlefs_rust::OpenFlags::READ)
                            {
                                if file.size() == 16384 {
                                    let buf = unsafe { &mut *core::ptr::addr_of_mut!(ROM_BUFFER) };
                                    if let Ok(n) = file.read(buf) {
                                        if n == 16384 {
                                            rom_name.clear();
                                            let _ = rom_name.push_str(name_str);
                                            info!("Loaded ROM: {}", name_str);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            fs.unmount().unwrap()
        }
        Err((_, storage)) => storage,
    };
    info!("Settings loaded. Drive ID: {}", drive_id);
    SNOOPER_CONFIG.update_device_id(drive_id);

    // Force RP2040 Timer to 1us ticks (12 cycles of 12MHz clk_ref)
    let (old_val, new_val) = unsafe {
        let timer_tick_reg = 0x4005802c as *mut u32;
        let old = core::ptr::read_volatile(timer_tick_reg);
        // Set to 12 cycles per tick and ENABLE bit (1 << 9 | 12 = 524)
        core::ptr::write_volatile(timer_tick_reg, (1 << 9) | 12);
        let new = core::ptr::read_volatile(timer_tick_reg);
        (old, new)
    };

    // Timer speed verification
    let t1 = embassy_rp::pac::TIMER0.timelr().read();
    cortex_m::asm::delay(10_000_000);
    let t2 = embassy_rp::pac::TIMER0.timelr().read();
    let startup_ticks = t2.wrapping_sub(t1);

    SNOOPER_CONFIG
        .diag_old_tick
        .store(old_val, Ordering::Relaxed);
    SNOOPER_CONFIG
        .diag_startup_ticks
        .store(startup_ticks, Ordering::Relaxed);
    info!(
        "Timer Diag: Old={} New={} StartupTicks={}",
        old_val, new_val, startup_ticks
    );

    // Initialize the shared double track buffers
    DUAL_CORE_BUFFER.reset(DUAL_CORE_DATA.buffers.get());
    let (manager_handle, emulator_handle) = DUAL_CORE_BUFFER.split();

    let stack = unsafe {
        let ptr: *mut Stack<1024> = core::ptr::addr_of_mut!(CORE1_STACK).cast();
        core::ptr::write(ptr, Stack::new());
        &mut *ptr
    };

    // Pack peripherals for Core 1
    let core1_pins = Core0Pins {
        i2c0: p.I2C0.into(),
        sda: p.PIN_4.into(),
        scl: p.PIN_5.into(),
        piezo: p.PIN_2.into(),
        rgb_red: p.PIN_16.into(),
        rgb_green: p.PIN_17.into(),
        rgb_blue: p.PIN_18.into(),
        spi1: p.SPI1.into(),
        clk: p.PIN_14.into(),
        mosi: p.PIN_15.into(),
        miso: p.PIN_12.into(),
        cs: p.PIN_13.into(),
        det: p.PIN_11.into(),
        btn0: p.PIN_6.into(),
        btn1: p.PIN_7.into(),
        btn2: p.PIN_8.into(),
        btn3: p.PIN_9.into(),
        pin25: p.PIN_25.into(),
    };

    // (track_addr unused)

    let hardware = pico1541_rp_common::emulation::EmulatorHardware {};

    let pio0 = embassy_rp::pio::Pio::new(p.PIO0, Irqs);

    info!("Starting Emulator on Core 1...");
    pico1541_rp_common::emulation::spawn_core1(
        p.CORE1.into(),
        stack,
        hardware,
        &SNOOPER_CONFIG,
        drive_id,
        pio0,
        embassy_rp::pio::Pio::new(p.PIO1, Irqs),
        emulator_handle,
        unsafe { &*core::ptr::addr_of!(ROM_BUFFER) },
        SYSTEM_CLOCK_FREQ,
    );

    // --- Run Manager on Core 0 ---
    spawner.spawn(
        pico1541_rp_common::tasks::settings_save_task(
            fs_storage_inst,
            pico1541_rp_common::tasks::RomBufferPtr(core::ptr::addr_of_mut!(ROM_BUFFER)),
            &SNOOPER_CONFIG,
            pico1541_manager::SETTINGS_CHANNEL.receiver(),
        )
        .unwrap(),
    );
    core0_init_task(
        spawner,
        core1_pins,
        &SNOOPER_CONFIG,
        rom_name,
        manager_handle,
    )
    .await;
}


#[embassy_executor::task]
async fn core0_perf_task(mut task: pico1541_manager::tasks::Core0PerfTask) {
    task.run().await;
}


#[embassy_executor::task]
async fn track_cache_run(
    mut task: TrackCacheTask<
        MyStorage,
        PlatformStorageManager,
        RpTrackHandoff,
        StatusLedsWrapper<CriticalSectionRawMutex>,
    >,
) {
    task.run().await;
}

#[embassy_executor::task]
async fn controller_run(
    mut task: MountControllerTask,
) {
    task.run().await;
}

#[embassy_executor::task]
async fn display_run(mut task: DisplayTask<DisplayWrapper<CriticalSectionRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn keyboard_run(mut task: KeyboardTask<DirectKeyboardInput>) {
    task.run().await;
}

#[embassy_executor::task]
async fn action_run(mut task: ActionTask) {
    task.run().await;
}

#[embassy_executor::task]
#[allow(clippy::type_complexity)]
async fn menu_run(
    mut task: MainMenuTask<
        DisplayWrapper<CriticalSectionRawMutex>,
        StatusLedsWrapper<CriticalSectionRawMutex>,
        IndicatorWrapper<CriticalSectionRawMutex>,
        StorageWrapper<MyRawBlockIo, MyDet>,
        LittleFsStorage,
    >,
) {
    task.run().await;
}

#[embassy_executor::task]
async fn blinky(pin: Peri<'static, embassy_rp::peripherals::PIN_25>) {
    let mut led = Output::new(pin, Level::Low);
    info!("Heartbeat blinky task started!");
    loop {
        led.set_high();
        embassy_time::Timer::after_millis(100).await;
        led.set_low();
        embassy_time::Timer::after_millis(900).await;
    }
}

#[embassy_executor::task]
async fn core1_load_run(mut task: pico1541_manager::tasks::Core1LoadTask) {
    task.run().await;
}

pub struct Core0Pins {
    pub i2c0: embassy_rp::Peri<'static, embassy_rp::peripherals::I2C0>,
    pub sda: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_4>,
    pub scl: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_5>,

    pub piezo: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_2>,

    pub rgb_red: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_16>,
    pub rgb_green: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_17>,
    pub rgb_blue: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_18>,

    pub spi1: embassy_rp::Peri<'static, embassy_rp::peripherals::SPI1>,
    pub clk: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_14>,
    pub mosi: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_15>,
    pub miso: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_12>,
    pub cs: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_13>,
    pub det: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_11>,

    pub btn0: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_6>,
    pub btn1: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_7>,
    pub btn2: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_8>,
    pub btn3: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_9>,

    pub pin25: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_25>,
}

#[embassy_executor::task]
async fn sd_card_monitor_task(
    mut task: pico1541_manager::tasks::SdMonitorTask<CriticalSectionRawMutex, MyRawBlockIo, MyDet>,
) {
    task.run().await;
}

async fn core0_init_task(
    spawner: Spawner,
    pins: Core0Pins,
    config: &'static SnooperConfig,
    rom_name: heapless::String<32>,
    manager_handle: ManagerHandle,
) {
    info!("core0_init_task started!");

    // 1. OLED Display (I2C)
    let sda = pins.sda;
    let scl = pins.scl;
    info!("Initializing I2C0...");
    let i2c = embassy_rp::i2c::I2c::new_async(pins.i2c0, scl, sda, Irqs, Config::default());

    static OLED_STATE: StaticCell<Mutex<CriticalSectionRawMutex, DisplayState>> = StaticCell::new();
    let state_obj = DisplayState::new();
    let state_mutex = OLED_STATE.init(Mutex::new(state_obj));

    static OLED_HW: StaticCell<Mutex<CriticalSectionRawMutex, DisplayHardware>> = StaticCell::new();
    let mut hw_obj = DisplayHardware::new(i2c);
    info!("Initializing OLED...");
    hw_obj.init();
    info!("OLED Initialized!");
    let hw_mutex = OLED_HW.init(Mutex::new(hw_obj));
    let display_wrapper = DisplayWrapper {
        state: state_mutex,
        hw: hw_mutex,
    };

    // 2. Indicators (Piezo + LEDs)
    let piezo = PiezoSpeaker::new(pins.piezo);
    let indicator = ActivityIndicator::new(piezo);

    static INDICATOR: StaticCell<Mutex<CriticalSectionRawMutex, ActivityIndicator>> =
        StaticCell::new();
    let indicator_mutex = INDICATOR.init(Mutex::new(indicator));

    // 3. RGB Status LEDs (Used in Hardware Test via Main Menu)
    let status_leds_obj = StatusLeds::new(pins.rgb_red, pins.rgb_green, pins.rgb_blue);
    static STATUS_LEDS: StaticCell<Mutex<CriticalSectionRawMutex, StatusLeds>> = StaticCell::new();
    let status_leds_mutex = STATUS_LEDS.init(Mutex::new(status_leds_obj));

    // 4. SD Card (SPI1)
    let cs = Output::new(pins.cs, Level::High);
    let det = Input::new(pins.det, Pull::Up);
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = 400_000;
    let spi = Spi::new_blocking(pins.spi1, pins.clk, pins.mosi, pins.miso, spi_config);

    let baudrate = option_env!("PICO1541_SD_BAUDRATE")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(25_000_000);

    static SD_CARD: StaticCell<SdCard<MySpi, MyCs, MyDelay>> = StaticCell::new();
    let card = SD_CARD.init(SdCard::new(spi, cs, embassy_time::Delay, baudrate));

    let block_io = SdBlockIo::new(card as *mut _);
    static SD_STORAGE: StaticCell<Mutex<CriticalSectionRawMutex, FatStorage<MyRawBlockIo, MyDet>>> =
        StaticCell::new();
    let sd_obj = FatStorage::new(block_io, det);
    let sd_mutex = SD_STORAGE.init(Mutex::new(sd_obj));

    let sd_task = pico1541_manager::tasks::SdMonitorTask::new(sd_mutex);
    spawner.spawn(sd_card_monitor_task(sd_task).unwrap());

    // 5. Keyboard Buttons
    let keyboard = DirectKeyboardInput::new(pins.btn0, pins.btn1, pins.btn2, pins.btn3);
    let keyboard_task = KeyboardTask::new(keyboard, pico1541_manager::KEYBOARD_CHANNEL.sender());

    let display_task = DisplayTask::new(display_wrapper.clone());


    let track_cache_task = TrackCacheTask::new(
        manager_handle,
        PlatformStorageManager {
            storage: StorageWrapper(sd_mutex),
            location: &CURRENT_LOCATION,
        },
        RpTrackHandoff::new(),
        &CURRENT_LOCATION,
        &MOTOR_STATE,
        StatusLedsWrapper(status_leds_mutex),
    );

    let mount_controller_task = MountControllerTask::new(
        &CURRENT_LOCATION,
        &MOTOR_STATE,
        CMD_CHANNEL.receiver(),
    );

    let ui_context = UiContext {
        action_rx: ACTION_CHANNEL.receiver(),
        cmd_tx: CMD_CHANNEL.sender(),
        settings_tx: SETTINGS_CHANNEL.sender(),
    };

    let main_menu_task = MainMenuTask::new(
        display_wrapper,
        StatusLedsWrapper(status_leds_mutex),
        IndicatorWrapper(indicator_mutex),
        StorageWrapper(sd_mutex),
        pico1541_rp_common::littlefs_storage::LittleFsStorage,
        &CURRENT_LOCATION,
        config,
        rom_name,
        SYSTEM_CLOCK_FREQ,
        ui_context,
        pico1541_common::sound::SoundSettings::default(),
    );

    spawner.spawn(menu_run(main_menu_task).unwrap());
    spawner.spawn(keyboard_run(keyboard_task).unwrap());
    let action_task = ActionTask::new(
        KEYBOARD_CHANNEL.receiver(),
        ACTION_CHANNEL.sender(),
    );
    spawner.spawn(action_run(action_task).unwrap());

    let perf_task = pico1541_manager::tasks::Core0PerfTask::new(&SNOOPER_CONFIG);
    spawner.spawn(core0_perf_task(perf_task).unwrap());
    spawner.spawn(display_run(display_task).unwrap());
    use embassy_rp::interrupt::{InterruptExt, Priority};
    embassy_rp::interrupt::TIMER0_IRQ_3.set_priority(Priority::P2);
    EXECUTOR_HIGH.start(embassy_rp::interrupt::TIMER0_IRQ_3);
    let spawner_high = EXECUTOR_HIGH.spawner();
    spawner_high.spawn(track_cache_run(track_cache_task).unwrap());
    spawner.spawn(controller_run(mount_controller_task).unwrap());
    spawner.spawn(blinky(pins.pin25).unwrap());
    let load_task = pico1541_manager::tasks::Core1LoadTask::new(config);
    spawner.spawn(core1_load_run(load_task).unwrap());
}

#[embassy_executor::task]
async fn usb_task(
    mut usb: embassy_usb::UsbDevice<
        'static,
        embassy_rp::usb::Driver<'static, embassy_rp::peripherals::USB>,
    >,
) {
    usb.run().await
}
