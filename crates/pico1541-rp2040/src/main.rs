#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

extern crate alloc;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

mod driver;

use defmt::info;
use defmt_rtt as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, InterruptHandler};
use embassy_rp::peripherals::{I2C0, SPI1, PIN_25, DMA_CH0, DMA_CH1};
use embassy_rp::spi::{Config as SpiConfig, Spi, Blocking};
use embassy_rp::multicore::Stack;
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;


use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler, SnooperConfig};
use pico1541_common::formats::AnyDiskImage;
use pico1541_common::storage::{PagedDirectorySource, MemoryStorage, RuntimeStorage};
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
use crate::driver::storage::{HadrisFileWrapper, SdStorage, StorageWrapper};
use embedded_hal_bus::spi::ExclusiveDevice;


use static_cell::StaticCell;
use portable_atomic::Ordering;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;
});

static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(8);

static CURRENT_DIR: Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)> = Mutex::new((heapless::String::new(), false));

// Core 1 Stack
static CORE1_STACK: StaticCell<Stack<16384>> = StaticCell::new();

pub struct LoggingSpiDevice<Spi> {
    inner: Spi,
}

impl<Spi> LoggingSpiDevice<Spi> {
    pub fn new(inner: Spi) -> Self {
        Self { inner }
    }
}

impl<Spi> embedded_hal::spi::ErrorType for LoggingSpiDevice<Spi>
where
    Spi: embedded_hal::spi::SpiDevice<u8>,
{
    type Error = Spi::Error;
}

impl<Spi> embedded_hal::spi::SpiDevice<u8> for LoggingSpiDevice<Spi>
where
    Spi: embedded_hal::spi::SpiDevice<u8>,
{
    fn transaction(&mut self, operations: &mut [embedded_hal::spi::Operation<'_, u8>]) -> Result<(), Self::Error> {
        let res = self.inner.transaction(operations);
        for (i, op) in operations.iter().enumerate() {
            match op {
                embedded_hal::spi::Operation::Read(buf) => {
                    defmt::info!("SPI Op {}: Read (len={}) {:?}", i, buf.len(), buf);
                }
                embedded_hal::spi::Operation::Write(buf) => {
                    defmt::info!("SPI Op {}: Write (len={}) {:?}", i, buf.len(), buf);
                }
                embedded_hal::spi::Operation::Transfer(read_buf, write_buf) => {
                    defmt::info!("SPI Op {}: Transfer (len={}) write {:?} | read {:?}", i, write_buf.len(), write_buf, read_buf);
                }
                embedded_hal::spi::Operation::TransferInPlace(buf) => {
                    defmt::info!("SPI Op {}: TransferInPlace (len={}) {:?}", i, buf.len(), buf);
                }
                embedded_hal::spi::Operation::DelayNs(ns) => {
                    defmt::info!("SPI Op {}: Delay {}ns", i, *ns);
                }
            }
        }
        res
    }
}

type MySpi = Spi<'static, SPI1, Blocking>;
type MyCs = Output<'static>;
type MyDelay = embassy_time::Delay;
type MySpiDev = LoggingSpiDevice<ExclusiveDevice<MySpi, MyCs, MyDelay>>;
type MyDet = Input<'static>;
type MyFile = HadrisFileWrapper<MySpiDev, MyDelay, MyDet>;
type MyStorage = RuntimeStorage<'static, MyFile>;
type MyDir = PagedDirectorySource<48>;
type MyDisk = AnyDiskImage<MyStorage, MyDir>;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 32768;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe {
            HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
        }
    }

    // Configure the system clock to 250 MHz
    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(250_000_000).unwrap();
    
    let p = embassy_rp::init(config);
    info!("Pico1541 Starting at 250MHz...");
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
let t1 = embassy_rp::pac::TIMER.timelr().read();
cortex_m::asm::delay(10_000_000); 
let t2 = embassy_rp::pac::TIMER.timelr().read();
let startup_ticks = t2.wrapping_sub(t1);

SNOOPER_CONFIG.diag_old_tick.store(old_val, Ordering::Relaxed);
SNOOPER_CONFIG.diag_startup_ticks.store(startup_ticks, Ordering::Relaxed);
info!("Timer Diag: Old={} New={} StartupTicks={}", old_val, new_val, startup_ticks);


    // Shared ActiveTrack buffer
    static ACTIVE_TRACK: StaticCell<ActiveTrack> = StaticCell::new();
    let track_ptr = ACTIVE_TRACK.init(ActiveTrack::new()) as *mut ActiveTrack;

    let stack = CORE1_STACK.init(Stack::new());

    // Pack peripherals for Core 1
    let core1_pins = Core1Pins {
        i2c0: p.I2C0.into(),
        sda: p.PIN_4.into(),
        scl: p.PIN_5.into(),
        piezo: p.PIN_2.into(),
        led_pin1: p.PIN_22.into(),
        led_pin2: p.PIN_3.into(),
        rgb_red: p.PIN_26.into(),
        rgb_green: p.PIN_27.into(),
        rgb_blue: p.PIN_28.into(),
        spi1: p.SPI1.into(),
        clk: p.PIN_14.into(),
        mosi: p.PIN_15.into(),
        miso: p.PIN_12.into(),
        cs: p.PIN_13.into(),
        det: p.PIN_11.into(),
        dma_ch0: p.DMA_CH0.into(),
        dma_ch1: p.DMA_CH1.into(),
        kbd_row1: p.PIN_6.into(),
        kbd_row2: p.PIN_7.into(),
        kbd_col1: p.PIN_8.into(),
        kbd_col2: p.PIN_9.into(),
        pin25: p.PIN_25.into(),
    };

    let track_addr = track_ptr as usize;

    // --- Spawn Core 1 ---
    embassy_rp::multicore::spawn_core1(p.CORE1, stack, move || {
        core1_main(
            core1_pins,
            track_addr,
            &SNOOPER_CONFIG,
        );
    });

    // IEC Pins
    let pins = IecPins {
        _data_rx: Input::new(p.PIN_16, Pull::None),
        _clk_rx: Input::new(p.PIN_17, Pull::None),
        _atn_rx: Input::new(p.PIN_18, Pull::None),
        _data_tx: Output::new(p.PIN_19, Level::Low),
        _clk_tx: Output::new(p.PIN_20, Level::Low),
    };

    let track_for_bridge = unsafe { &mut *track_ptr };
    let bridge = HardwareBridge::new(pins, track_for_bridge, &SNOOPER_CONFIG);

    let rom = include_bytes!("../../../roms/1541-II.251968-03.bin");
    let mut drive = Emulator::new(*rom, 8);
    drive.reset();

    emulator_loop(drive, bridge);
}

struct IecPins {
    _data_rx: Input<'static>,
    _clk_rx: Input<'static>,
    _atn_rx: Input<'static>,
    _data_tx: Output<'static>,
    _clk_tx: Output<'static>,
}

struct HardwareBridge {
    track: &'static mut ActiveTrack,
    _pins: IecPins,
    snooper: IecSnooper,
    handler: IecSnoopByteHandler,
    config: &'static SnooperConfig,
}

impl HardwareBridge {
    fn new(pins: IecPins, track: &'static mut ActiveTrack, config: &'static SnooperConfig) -> Self {
        let device_id = config.device_id.load(Ordering::Relaxed);
        Self {
            track,
            _pins: pins,
            snooper: IecSnooper::new(device_id),
            handler: IecSnoopByteHandler::new(),
            config,
        }
    }
}

impl IecBus for HardwareBridge {
    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn iec_host(&self) -> IecBusState {
        // Direct SIO register read for speed.
        // GPIO_IN at 0xd0000004
        // Pins 16, 17, 18 are DATA, CLK, ATN
        let pins = unsafe { core::ptr::read_volatile(0xd0000004 as *const u32) };
        IecBusState(((pins >> 16) & 0x07) as u8)
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn iec_drive(&mut self, state: IecBusState) {
        // GPIO 19: DATA_TX (bit 0), 20: CLK_TX (bit 1)
        let mut mask = (state.0 as u32 & 0x03) << 19;
        
        // Use a logical bitwise op instead of an if-branch for should_drive_data.
        // This is safe because Relaxed load is just an ldr instruction on RP2040.
        let hijack_force = (self.config.should_drive_data.load(Ordering::Relaxed) as u32) << 19;
        mask |= hijack_force;
        
        unsafe {
            // SIO GPIO_OUT_SET (0x14) and GPIO_OUT_CLR (0x18)
            const SIO_SET: *mut u32 = 0xd0000014 as *mut u32;
            const SIO_CLR: *mut u32 = 0xd0000018 as *mut u32;
            const DATA_CLK_MASK: u32 = 0x03 << 19;
            
            core::ptr::write_volatile(SIO_SET, mask);
            core::ptr::write_volatile(SIO_CLR, !mask & DATA_CLK_MASK);
        }
    }
}

impl HostInterface for HardwareBridge {
    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        self.track
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn send_event(&mut self, event: DriveEvent) {
        // try_send is non-blocking and safe for Core 0.
        let _ = pico1541_manager::EVENT_CHANNEL.try_send(event);
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(Ordering::Relaxed)
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn pop_discard_request(&mut self) -> bool {
        self.config.discard_request.swap(false, Ordering::Relaxed)
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, Ordering::Relaxed);
        self.config.talk_addr.store(talk, Ordering::Relaxed);
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn get_device_id(&self) -> u8 {
        self.config.device_id.load(Ordering::Relaxed)
    }

    #[link_section = ".ramfunc"]
    #[inline(always)]
    fn sync(&mut self, host: IecBusState) {
        // The snooper is only updated if something changed on the bus.
        // This call is fast in the "no change" case inside the snooper itself.
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

struct MyOpener {
    storage: StorageWrapper<MySpiDev, MyDelay, MyDet>,
}

impl pico1541_manager::tasks::virtual_dir::FileOpener for MyOpener {
    type FileStorage = MyStorage;

    async fn open_file(&self, name: &str) -> Result<Self::FileStorage, pico1541_common::formats::Error> {
        let (dir_path, use_internal) = {
            let guard = CURRENT_DIR.lock().await;
            (guard.0.clone(), guard.1)
        };
        
        let mut full_path = heapless::String::<128>::new();
        if dir_path == "/" || dir_path.is_empty() {
            let _ = full_path.push_str("/");
            let _ = full_path.push_str(name);
        } else {
            let _ = full_path.push_str(&dir_path);
            let _ = full_path.push_str("/");
            let _ = full_path.push_str(name);
        }
        
        if use_internal {
            for file in pico1541_manager::driver::internal_storage::INTERNAL_FILES {
                if file.name.eq_ignore_ascii_case(name) {
                    return Ok(RuntimeStorage::Memory(MemoryStorage::new(file.data)));
                }
            }
            Err(pico1541_common::formats::Error::Io)
        } else {
            use pico1541_manager::driver::FileStorageDriver;
            let file_handle = self.storage.open_file_handle(full_path.as_str());
            Ok(RuntimeStorage::File(file_handle))
        }
    }

    async fn count_dir_entries(&self, path: &str, use_internal: bool) -> Result<usize, pico1541_common::formats::Error> {
        use pico1541_manager::driver::FileStorageDriver;
        if use_internal {
            let mut internal = pico1541_manager::driver::InternalStorage;
            internal.count_dir_entries(path).await
        } else {
            let mut storage = self.storage.clone();
            storage.count_dir_entries(path).await
        }
    }

    async fn list_dir_range(
        &self,
        path: &str,
        use_internal: bool,
        start_index: usize,
        count: usize,
        mut callback: impl FnMut(pico1541_common::storage::DirectoryEntry),
    ) -> Result<(), pico1541_common::formats::Error> {
        use pico1541_manager::driver::FileStorageDriver;
        if use_internal {
            let mut internal = pico1541_manager::driver::InternalStorage;
            internal.list_dir_range(path, start_index, count, |info| {
                let name_len = info.name.iter().position(|&c| c == 0).unwrap_or(info.name.len());
                let entry = pico1541_common::storage::DirectoryEntry::from_file_info(
                    &info.name[..name_len],
                    info.is_dir,
                    info.size,
                );
                callback(entry);
            }).await
        } else {
            let mut storage = self.storage.clone();
            storage.list_dir_range(path, start_index, count, |info| {
                let name_len = info.name.iter().position(|&c| c == 0).unwrap_or(info.name.len());
                let entry = pico1541_common::storage::DirectoryEntry::from_file_info(
                    &info.name[..name_len],
                    info.is_dir,
                    info.size,
                );
                callback(entry);
            }).await
        }
    }
}

#[embassy_executor::task]
async fn vdisk_run(mut task: VirtualDiskTask<'static, MyStorage, MyDir, MyOpener>) {
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
        StorageWrapper<MySpiDev, MyDelay, MyDet>,
        MyFile,
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

#[embassy_executor::task]
async fn core1_load_run(config: &'static SnooperConfig) {
    let mut iterations = 0u32;
    let mut last_time = embassy_time::Instant::now();
    
    loop {
        iterations += 1;
        // Yield to let other tasks run, but don't do it every microsecond!
        // Using a longer delay significantly reduces context switching overhead.
        embassy_time::Timer::after_millis(10).await;
        
        let now = embassy_time::Instant::now();
        let elapsed = now.duration_since(last_time).as_millis();
        if elapsed >= 1000 {
            let old_avg = config.core1_avg_raw.load(Ordering::Relaxed);
            let new_avg = if old_avg == 0 { iterations } else { (iterations + old_avg * 3) / 4 };
            config.core1_avg_raw.store(new_avg, Ordering::Relaxed);

            let max_raw = config.core1_max_raw.load(Ordering::Relaxed);
            if iterations > max_raw {
                config.core1_max_raw.store(iterations, Ordering::Relaxed);
            }
            
            iterations = 0;
            last_time = now;
        }
    }
}

pub struct Core1Pins {
    pub i2c0: embassy_rp::Peri<'static, embassy_rp::peripherals::I2C0>,
    pub sda: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_4>,
    pub scl: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_5>,

    pub piezo: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_2>,
    pub led_pin1: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_22>,
    pub led_pin2: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_3>,

    pub rgb_red: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_26>,
    pub rgb_green: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_27>,
    pub rgb_blue: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_28>,

    pub spi1: embassy_rp::Peri<'static, embassy_rp::peripherals::SPI1>,
    pub clk: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_14>,
    pub mosi: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_15>,
    pub miso: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_12>,
    pub cs: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_13>,
    pub det: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_11>,
    pub dma_ch0: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH0>,
    pub dma_ch1: embassy_rp::Peri<'static, embassy_rp::peripherals::DMA_CH1>,

    pub kbd_row1: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_6>,
    pub kbd_row2: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_7>,
    pub kbd_col1: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_8>,
    pub kbd_col2: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_9>,

    pub pin25: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_25>,
}

#[embassy_executor::task]
async fn core1_init_task(
    spawner: Spawner,
    pins: Core1Pins,
    track_addr: usize,
    config: &'static SnooperConfig,
) {
    let track_ptr = track_addr as *mut ActiveTrack;

    // 1. OLED Display (I2C)
    let sda = pins.sda;
    let scl = pins.scl;
    let i2c = embassy_rp::i2c::I2c::new_async(pins.i2c0, scl, sda, Irqs, Config::default());

    static OLED: StaticCell<Mutex<CriticalSectionRawMutex, OledDisplay>> = StaticCell::new();
    let mut oled_obj = OledDisplay::new(i2c);
    oled_obj.init().await;
    let oled_mutex = OLED.init(Mutex::new(oled_obj));

    // 2. Indicators (Piezo + LEDs)
    let activity_led = ActivityLed::new(pins.led_pin1, pins.led_pin2);
    let piezo = PiezoSpeaker::new(pins.piezo);
    let indicator = ActivityIndicator::new(activity_led, piezo);

    static INDICATOR: StaticCell<Mutex<CriticalSectionRawMutex, ActivityIndicator>> =
        StaticCell::new();
    let indicator_mutex = INDICATOR.init(Mutex::new(indicator));
    let activity_task = DriveActivityTask::new(IndicatorWrapper(indicator_mutex));

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
    let spi_dev = ExclusiveDevice::new(spi, cs, embassy_time::Delay).unwrap();
    let logging_spi_dev = LoggingSpiDevice::new(spi_dev);

    static SD_STORAGE: StaticCell<Mutex<CriticalSectionRawMutex, SdStorage<MySpiDev, MyDelay, MyDet>>> = StaticCell::new();
    let sd_obj = SdStorage::new(logging_spi_dev, embassy_time::Delay, det);
    let sd_mutex = SD_STORAGE.init(Mutex::new(sd_obj));

    // 5. Keyboard Matrix
    let keyboard = KeyboardInput::new(pins.kbd_row1, pins.kbd_row2, pins.kbd_col1, pins.kbd_col2);
    let keyboard_task = KeyboardTask::new(keyboard);

    // 6. Display Task
    let display_task = DisplayTask::new(DisplayWrapper(oled_mutex));

    // 7. Snooper Task
    let snooper_task = SnooperTask::new(8);

    // 8. Storage (Starts unmounted)
    static DISK_IMAGE: StaticCell<Mutex<CriticalSectionRawMutex, Option<MyDisk>>> = StaticCell::new();
    let disk_image_mutex = DISK_IMAGE.init(Mutex::new(None));

    let track_for_task = unsafe { &mut *track_ptr };
    let vdisk_task = VirtualDiskTask::new(
        disk_image_mutex,
        track_for_task,
        MyOpener { storage: StorageWrapper(sd_mutex) },
        &CURRENT_DIR,
    );

    let menu_task = MainMenuTask::new(
        DisplayWrapper(oled_mutex),
        StatusLedsWrapper(status_leds_mutex),
        IndicatorWrapper(indicator_mutex),
        StorageWrapper(sd_mutex),
        disk_image_mutex,
        &CURRENT_DIR,
        config,
    );

    spawner.spawn(menu_run(menu_task).unwrap());
    spawner.spawn(activity_run(activity_task).unwrap());
    spawner.spawn(keyboard_run(keyboard_task).unwrap());
    spawner.spawn(display_run(display_task).unwrap());
    spawner.spawn(snooper_run(snooper_task).unwrap());
    spawner.spawn(vdisk_run(vdisk_task).unwrap());
    spawner.spawn(blinky(pins.pin25).unwrap());
    spawner.spawn(core1_load_run(config).unwrap());
}

fn core1_main(
    pins: Core1Pins,
    track_addr: usize,
    config: &'static SnooperConfig,
) -> ! {
    static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();
    let executor = EXECUTOR.init(embassy_executor::Executor::new());

    executor.run(|spawner| {
        spawner.spawn(core1_init_task(spawner, pins, track_addr, config).unwrap());
    })
}

#[link_section = ".ramfunc"]
fn emulator_loop(mut drive: Emulator<HardwareBridge>, mut bridge: HardwareBridge) -> ! {
    let timer = embassy_rp::pac::TIMER;
    let mut last_tick = timer.timelr().read();
    let mut cycle_counter = 0u32;
    let mut last_perf_update = last_tick;
    let mut idle_counter = 0u32;

    loop {
        let now = timer.timelr().read();
        
        let mut caught_up = 0;
        while (now.wrapping_sub(last_tick) as i32) > 0 && caught_up < 100 {
            last_tick = last_tick.wrapping_add(1);
            drive.tick(&mut bridge);
            cycle_counter += 1;
            caught_up += 1;
        }

        // Update performance metrics every second
        if now.wrapping_sub(last_perf_update) >= 1_000_000 {
            let config = bridge.config;
            let old_avg = config.core0_avg_raw.load(Ordering::Relaxed);
            // EMA on the idle counter, not the cycle counter!
            let new_avg = if old_avg == 0 { idle_counter } else { (idle_counter + old_avg * 3) / 4 };
            
            config.emu_speed_hz.store(cycle_counter, Ordering::Relaxed);
            config.core0_avg_raw.store(new_avg, Ordering::Relaxed);

            // Calibrate max idle capacity
            let max_raw = config.core0_max_raw.load(Ordering::Relaxed);
            if idle_counter > max_raw {
                config.core0_max_raw.store(idle_counter, Ordering::Relaxed);
            }
            
            cycle_counter = 0;
            idle_counter = 0;
            last_perf_update = now;
        }

        if caught_up == 0 {
            idle_counter = idle_counter.wrapping_add(1);
            cortex_m::asm::nop();
        }
    }
}
