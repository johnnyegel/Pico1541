#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

extern crate alloc;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

mod driver;
mod flash_storage;

use core::sync::atomic::Ordering;
use defmt::{info, error};
use defmt_rtt as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, InterruptHandler};
use embassy_rp::peripherals::{I2C0, SPI1, DMA_CH0, DMA_CH1};
use embassy_rp::spi::{Config as SpiConfig, Spi, Blocking};
use embassy_rp::multicore::Stack;
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, IecBusState};
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler, SnooperConfig};
use pico1541_common::formats::AnyDiskImage;
use pico1541_common::storage::{PagedDirectorySource, RuntimeStorage};
use pico1541_emulator::Emulator;
use pico1541_manager::tasks::{
    ActionTask, DisplayTask, DriveActivityTask, KeyboardTask, MainMenuTask, SnooperTask, VirtualDiskTask,
};
use panic_probe as _;

#[cfg(has_mklittlefs)]
#[link_section = ".fs"]
#[used]
static FS_IMAGE: [u8; 512 * 1024] = *include_bytes!(concat!(env!("OUT_DIR"), "/fs.bin"));

use crate::driver::display::{DisplayWrapper, OledDisplay};
use crate::driver::indicator::{ActivityIndicator, IndicatorWrapper};
use crate::driver::input::KeyboardInput;
use crate::driver::leds::{ActivityLed, StatusLeds, StatusLedsWrapper};
use crate::driver::speaker::PiezoSpeaker;
use crate::driver::blockdevsdcard::{SdCard, SdBlockIo};
use pico1541_manager::driver::file_storage::{HadrisFileWrapper, FatStorage, StorageWrapper};


use static_cell::StaticCell;
pub mod littlefs_storage;
use littlefs_storage::LittleFsStorage;
use flash_storage::FlashStorage;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;
});

const DEFAULT_DRIVE_ID: u8 = 9;


static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(DEFAULT_DRIVE_ID);

static CURRENT_DIR: Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)> = Mutex::new((heapless::String::new(), false));

// Core 1 Stack
#[link_section = ".core1_stack"]
static mut CORE1_STACK: core::mem::MaybeUninit<Stack<40960>> = core::mem::MaybeUninit::uninit();

// ROM Buffer for dynamic loading
#[link_section = ".sram3"]
static mut ROM_BUFFER: [u8; 16384] = [0; 16384];

type MySpi = Spi<'static, SPI1, Blocking>;

impl crate::driver::blockdevsdcard::SpiFreq for MySpi {
    fn set_freq(&mut self, freq: u32) {
        self.set_frequency(freq);
    }
}
type MyCs = Output<'static>;
type MyDelay = embassy_time::Delay;
type MyDet = Input<'static>;
type MyRawBlockIo = SdBlockIo<MySpi, MyCs, MyDelay>;
type MyFile = HadrisFileWrapper<MyRawBlockIo, MyDet>;
type MyStorage = RuntimeStorage<'static, MyFile, littlefs_storage::LittleFsFileHandle>;
type MyDir = PagedDirectorySource<48>;
type MyDisk = AnyDiskImage<MyStorage, MyDir>;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    {
        use core::mem::MaybeUninit;
        const HEAP_SIZE: usize = 32768;
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe {
            HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE);
        }
    }

    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(250_000_000).unwrap();
    let p = embassy_rp::init(config);
    info!("Pico1541 Starting at 250MHz...");

    // Initialize Flash for LittleFS
    // RP2040 has 2MB of flash. We reserved 1MB at the end.
    let flash_driver = embassy_rp::flash::Flash::<_, embassy_rp::flash::Blocking, { 2 * 1024 * 1024 }>::new_blocking(p.FLASH);
    let fs_storage_inst = FlashStorage::new(flash_driver);
    
    fn fs_config() -> littlefs_rust::Config {
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        config
    }
    
    // Attempt to mount LittleFS, if it fails format it
    let mut fs_storage_inst = match littlefs_rust::Filesystem::mount(fs_storage_inst, fs_config()) {
        Ok(fs) => fs.unmount().unwrap_or_else(|e| panic!("failed to unmount: {:?}", e)),
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
                        if let Ok(rom_file) = fs.open(path.as_str(), littlefs_rust::OpenFlags::READ) {
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
                    for entry in dir {
                        if let Ok(entry) = entry {
                            let name_str = entry.name.as_str();
                            if name_str.ends_with(".bin") || name_str.ends_with(".BIN") || name_str.ends_with(".rom") || name_str.ends_with(".ROM") {
                                let mut path = heapless::String::<64>::new();
                                let _ = path.push_str("/roms/");
                                let _ = path.push_str(name_str);
                                if let Ok(file) = fs.open(path.as_str(), littlefs_rust::OpenFlags::READ) {
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
            }

            fs.unmount().unwrap()
        }
        Err((_, storage)) => storage,
    };
    info!("Settings loaded. Drive ID: {}", drive_id);
    SNOOPER_CONFIG.device_id.store(drive_id, Ordering::Relaxed);

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
    static mut ACTIVE_TRACK: core::mem::MaybeUninit<ActiveTrack> = core::mem::MaybeUninit::uninit();
    let track_ptr = unsafe {
        ACTIVE_TRACK.as_mut_ptr().write(ActiveTrack::new());
        ACTIVE_TRACK.as_mut_ptr()
    };

    let stack = unsafe {
        CORE1_STACK.as_mut_ptr().write(Stack::new());
        &mut *CORE1_STACK.as_mut_ptr()
    };

    // Pack peripherals for Core 1
    let core1_pins = Core0Pins {
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

    // IEC Pins
    let pins = IecPins {
        _data_rx: Input::new(p.PIN_16, Pull::None),
        _clk_rx: Input::new(p.PIN_17, Pull::None),
        _atn_rx: Input::new(p.PIN_18, Pull::None),
        _data_tx: Output::new(p.PIN_19, Level::Low),
        _clk_tx: Output::new(p.PIN_20, Level::Low),
        reset_rx: Input::new(p.PIN_21, Pull::Up),
    };

    let track_for_bridge = unsafe { &mut *track_ptr };
    let mut bridge = HardwareBridge::new(pins, track_for_bridge, &SNOOPER_CONFIG);

    // --- Spawn Core 1 ---
    info!("Starting Emulator on Core 1...");
    embassy_rp::multicore::spawn_core1(p.CORE1, stack, move || {
        let mut drive = Emulator::new(unsafe { &ROM_BUFFER }, drive_id);
        drive.reset(&mut bridge);
        emulator_loop(drive, bridge);
    });

    // --- Run Manager on Core 0 ---
    spawner.spawn(settings_save_task(fs_storage_inst).unwrap());
    core0_init_task(spawner, core1_pins, track_addr, &SNOOPER_CONFIG, rom_name).await;
}

struct IecPins {
    _data_rx: Input<'static>,
    _clk_rx: Input<'static>,
    _atn_rx: Input<'static>,
    _data_tx: Output<'static>,
    _clk_tx: Output<'static>,
    reset_rx: Input<'static>,
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
    storage: StorageWrapper<MyRawBlockIo, MyDet>,
}

impl pico1541_manager::tasks::virtual_dir::FileOpener for MyOpener {
    type FileStorage = MyStorage;

    async fn open_file(&self, requested_petscii: &[u8]) -> Result<Self::FileStorage, pico1541_common::formats::Error> {
        let (dir_path, use_internal) = {
            let guard = CURRENT_DIR.lock().await;
            (guard.0.clone(), guard.1)
        };
        
        let mut found_filename = heapless::String::<64>::new();
        
        if use_internal {
            use pico1541_manager::driver::FileStorageDriver;
            let mut internal = littlefs_storage::LittleFsStorage;
            
            let mut found = false;
            let _ = internal.list_dir_range(dir_path.as_str(), 0, 1000, |info| {
                if !found {
                    let mangled = pico1541_common::storage::mangle_filename(&info.name);
                    if pico1541_manager::tasks::virtual_dir::match_c64_filename(&mangled, requested_petscii) {
                        let name_len = info.name.iter().position(|&c| c == 0).unwrap_or(info.name.len());
                        if let Ok(s) = core::str::from_utf8(&info.name[..name_len]) {
                            let _ = found_filename.push_str(s);
                            found = true;
                        }
                    }
                }
            }).await;
            
            if !found {
                return Err(pico1541_common::formats::Error::FileNotFound);
            }
            
            let mut full_path = heapless::String::<128>::new();
            if dir_path == "/" || dir_path.is_empty() {
                let _ = full_path.push_str("/");
                let _ = full_path.push_str(found_filename.as_str());
            } else {
                let _ = full_path.push_str(&dir_path);
                let _ = full_path.push_str("/");
                let _ = full_path.push_str(found_filename.as_str());
            }
            
            let file_handle = internal.open_file_handle(full_path.as_str());
            Ok(RuntimeStorage::File2(file_handle))
        } else {
            use pico1541_manager::driver::FileStorageDriver;
            let mut sd = self.storage.clone();
            
            let mut found = false;
            let _ = sd.list_dir_range(dir_path.as_str(), 0, 1000, |info| {
                if !found {
                    let mangled = pico1541_common::storage::mangle_filename(&info.name);
                    if pico1541_manager::tasks::virtual_dir::match_c64_filename(&mangled, requested_petscii) {
                        let name_len = info.name.iter().position(|&c| c == 0).unwrap_or(info.name.len());
                        if let Ok(s) = core::str::from_utf8(&info.name[..name_len]) {
                            let _ = found_filename.push_str(s);
                            found = true;
                        }
                    }
                }
            }).await;
            
            if !found {
                return Err(pico1541_common::formats::Error::FileNotFound);
            }
            
            let mut full_path = heapless::String::<128>::new();
            if dir_path == "/" || dir_path.is_empty() {
                let _ = full_path.push_str("/");
                let _ = full_path.push_str(found_filename.as_str());
            } else {
                let _ = full_path.push_str(&dir_path);
                let _ = full_path.push_str("/");
                let _ = full_path.push_str(found_filename.as_str());
            }
            
            let file_handle = sd.open_file_handle(full_path.as_str());
            Ok(RuntimeStorage::File(file_handle))
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
            let mut internal = littlefs_storage::LittleFsStorage;
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
async fn action_run(mut task: ActionTask) {
    task.run().await;
}


#[embassy_executor::task]
async fn menu_run(
    mut task: MainMenuTask<
        DisplayWrapper<CriticalSectionRawMutex>,
        StatusLedsWrapper<CriticalSectionRawMutex>,
        IndicatorWrapper<CriticalSectionRawMutex>,
        StorageWrapper<MyRawBlockIo, MyDet>,
        LittleFsStorage,
        MyFile,
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

pub struct Core0Pins {
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
async fn sd_card_monitor_task(
    sd_mutex: &'static Mutex<CriticalSectionRawMutex, FatStorage<MyRawBlockIo, MyDet>>
) {
    loop {
        let mut mounted = false;
        {
            let mut storage = sd_mutex.lock().await;
            if storage.det_pin.is_low() && storage.fat_fs.is_none() {
                if storage.ensure_mounted().await.is_ok() {
                    mounted = true;
                }
            } else if storage.det_pin.is_high() {
                storage.fat_fs = None;
                storage.card_init = false;
                storage.block_dev.reset();
            }
        }
        if !mounted {
            embassy_time::Timer::after(embassy_time::Duration::from_millis(50)).await;
        } else {
            embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
        }
    }
}

async fn core0_init_task(
    spawner: Spawner,
    pins: Core0Pins,
    track_addr: usize,
    config: &'static SnooperConfig,
    rom_name: heapless::String<32>,
) {
    info!("core0_init_task started!");
    let track_ptr = track_addr as *mut ActiveTrack;

    // 1. OLED Display (I2C)
    let sda = pins.sda;
    let scl = pins.scl;
    info!("Initializing I2C0...");
    let i2c = embassy_rp::i2c::I2c::new_async(pins.i2c0, scl, sda, Irqs, Config::default());

    static OLED: StaticCell<Mutex<CriticalSectionRawMutex, OledDisplay>> = StaticCell::new();
    let mut oled_obj = OledDisplay::new(i2c);
    info!("Initializing OLED...");
    oled_obj.init().await;
    info!("OLED Initialized!");
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
    
    let baudrate = option_env!("PICO1541_SD_BAUDRATE")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(25_000_000);
        
    static SD_CARD: StaticCell<SdCard<MySpi, MyCs, MyDelay>> = StaticCell::new();
    let card = SD_CARD.init(SdCard::new(spi, cs, embassy_time::Delay, baudrate));

    let block_io = SdBlockIo::new(card as *mut _);
    static SD_STORAGE: StaticCell<Mutex<CriticalSectionRawMutex, FatStorage<MyRawBlockIo, MyDet>>> = StaticCell::new();
    let sd_obj = FatStorage::new(block_io, det);
    let sd_mutex = SD_STORAGE.init(Mutex::new(sd_obj));

    spawner.spawn(sd_card_monitor_task(sd_mutex).unwrap());

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
        &*disk_image_mutex,
        track_for_task,
        MyOpener { storage: StorageWrapper(sd_mutex) },
        &CURRENT_DIR,
    );

    let main_menu_task = MainMenuTask::new(
        DisplayWrapper(oled_mutex),
        StatusLedsWrapper(status_leds_mutex),
        IndicatorWrapper(indicator_mutex),
        StorageWrapper(sd_mutex),
        LittleFsStorage,
        &*disk_image_mutex,
        &CURRENT_DIR,
        config,
        rom_name,
    );

    spawner.spawn(menu_run(main_menu_task).unwrap());
    spawner.spawn(activity_run(activity_task).unwrap());
    spawner.spawn(keyboard_run(keyboard_task).unwrap());
    let action_task = ActionTask::new();
    spawner.spawn(action_run(action_task).unwrap());


    spawner.spawn(display_run(display_task).unwrap());
    spawner.spawn(snooper_run(snooper_task).unwrap());
    spawner.spawn(vdisk_run(vdisk_task).unwrap());
    spawner.spawn(blinky(pins.pin25).unwrap());
    spawner.spawn(core1_load_run(config).unwrap());
}

#[embassy_executor::task]
async fn settings_save_task(mut flash_storage: FlashStorage<'static>) {
    loop {
        let cmd = pico1541_manager::SETTINGS_CHANNEL.receive().await;
        
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        
        match cmd {
                pico1541_manager::SettingsCommand::SaveDriveId(id) => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            if let Ok(file) = fs.open("/settings.dat", littlefs_rust::OpenFlags::WRITE | littlefs_rust::OpenFlags::CREATE | littlefs_rust::OpenFlags::TRUNC) {
                                let _ = file.write(&[id]);
                                let _ = file.sync();
                                info!("Successfully wrote /settings.dat: Drive ID {}", id);
                            }
                            flash_storage = fs.unmount().unwrap();
                        }
                        Err((_, storage)) => {
                            flash_storage = storage;
                        }
                    }
                }
                pico1541_manager::SettingsCommand::SaveBootRom(name) => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            let _ = fs.mkdir("/settings");
                            if let Ok(file) = fs.open("/settings/boot_rom.txt", littlefs_rust::OpenFlags::WRITE | littlefs_rust::OpenFlags::CREATE | littlefs_rust::OpenFlags::TRUNC) {
                                let _ = file.write(name.as_bytes());
                                let _ = file.sync();
                                info!("Successfully wrote /settings/boot_rom.txt: {}", name.as_str());
                            }
                            flash_storage = fs.unmount().unwrap();
                        }
                        Err((_, storage)) => {
                            flash_storage = storage;
                        }
                    }
                }
                pico1541_manager::SettingsCommand::DeleteRom(name) => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            let mut full_path = heapless::String::<128>::new();
                            let _ = full_path.push_str("/roms/");
                            let _ = full_path.push_str(name.as_str());
                            let _ = fs.remove(full_path.as_str());
                            info!("Deleted ROM: {}", full_path.as_str());
                            flash_storage = fs.unmount().unwrap();
                        }
                        Err((_, storage)) => {
                            flash_storage = storage;
                        }
                    }
                }
                pico1541_manager::SettingsCommand::InstallRomStart { filename, size: _, offset: _ } => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            let _ = fs.mkdir("/roms");
                            let mut full_path = heapless::String::<128>::new();
                            let _ = full_path.push_str("/roms/");
                            let _ = full_path.push_str(filename.as_str());
                            
                            if let Ok(file) = fs.open(full_path.as_str(), littlefs_rust::OpenFlags::WRITE | littlefs_rust::OpenFlags::CREATE | littlefs_rust::OpenFlags::TRUNC) {
                                info!("Opened {} for writing", full_path.as_str());
                                loop {
                                    let inner_cmd = pico1541_manager::SETTINGS_CHANNEL.receive().await;
                                    match inner_cmd {
                                        pico1541_manager::SettingsCommand::InstallRomChunk(data) => {
                                            let _ = file.write(&data);
                                        }
                                        pico1541_manager::SettingsCommand::InstallRomFinish => {
                                            let _ = file.sync();
                                            info!("Finished installing ROM");
                                            break;
                                        }
                                        _ => {
                                            // Ignore other commands during installation
                                        }
                                    }
                                }
                            }
                            flash_storage = fs.unmount().unwrap();
                        }
                        Err((_, storage)) => {
                            flash_storage = storage;
                        }
                    }
                }
                pico1541_manager::SettingsCommand::LoadRomToMemory(name) => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            let mut full_path = heapless::String::<128>::new();
                            let _ = full_path.push_str("/roms/");
                            let _ = full_path.push_str(name.as_str());
                            crate::SNOOPER_CONFIG.halt_emulator.store(true, portable_atomic::Ordering::Relaxed);
                            if let Ok(file) = fs.open(full_path.as_str(), littlefs_rust::OpenFlags::READ) {
                                unsafe {
                                    let mut bytes_read = 0;
                                    while bytes_read < 16384 {
                                        if let Ok(n) = file.read(&mut ROM_BUFFER[bytes_read..]) {
                                            if n == 0 { break; }
                                            bytes_read += n as usize;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                info!("Loaded ROM {} into memory", name.as_str());
                                crate::SNOOPER_CONFIG.trigger_reset.store(true, portable_atomic::Ordering::Relaxed);
                            }
                            crate::SNOOPER_CONFIG.halt_emulator.store(false, portable_atomic::Ordering::Relaxed);
                            flash_storage = fs.unmount().unwrap();
                        }
                        Err((_, storage)) => {
                            flash_storage = storage;
                        }
                    }
                }
                _ => {}
        }
    }
}

#[link_section = ".ramfunc"]
fn emulator_loop(mut drive: Emulator<HardwareBridge>, mut bridge: HardwareBridge) -> ! {
    let timer = embassy_rp::pac::TIMER;
    let mut idle_counter = 0u32;
    let mut cycle_counter = 0u32;

    let mut syst = unsafe { cortex_m::Peripherals::steal().SYST };
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(0x00FFFFFF);
    syst.clear_current();
    syst.enable_counter();

    // Calibrate true max idle for 10ms to calculate accurate CPU load
    let calib_start = timer.timelr().read();
    let mut calib_last_tick = calib_start;
    let mut calib_last_perf_update = calib_start;
    let mut calib_idle = 0u32;
    loop {
        let now = timer.timelr().read();
        if now.wrapping_sub(calib_start) >= 10_000 {
            break;
        }
        
        if bridge.config.halt_emulator.load(Ordering::Relaxed) { continue; }
        if bridge.config.trigger_reset.load(Ordering::Relaxed) { continue; }

        let mut caught_up = 0;
        // Simulate the catchup loop overhead without doing tick()
        while (now.wrapping_sub(calib_last_tick) as i32) > 0 && caught_up < 100 {
            calib_last_tick = calib_last_tick.wrapping_add(1);
            // drive.tick(&mut bridge);  <-- omitted to find max idle
            caught_up += 1;
        }

        if now.wrapping_sub(calib_last_perf_update) >= 1_000_000 {
            calib_last_perf_update = now;
        } else {
            calib_idle = calib_idle.wrapping_add(1);
            cortex_m::asm::nop();
        }
    }
    bridge.config.core0_max_raw.store(calib_idle * 100, Ordering::Relaxed);

    let mut last_tick = timer.timelr().read();
    let mut last_perf_update = last_tick;
    let mut was_reset = false;

    let mut cpu_cycles_acc = 0u64;
    let mut ticks_acc = 0u64;

    loop {
        let now = timer.timelr().read();
        
        if bridge.config.halt_emulator.load(Ordering::Relaxed) {
            cortex_m::asm::delay(100);
            last_tick = timer.timelr().read();
            continue;
        }

        if bridge.config.trigger_reset.load(Ordering::Relaxed) {
            drive.update_rom(unsafe { &ROM_BUFFER });
            drive.reset(&mut bridge);
            bridge.config.trigger_reset.store(false, Ordering::Relaxed);
        }

        let mut caught_up = 0;
        let is_reset = bridge._pins.reset_rx.is_high();

        while (now.wrapping_sub(last_tick) as i32) > 0 && caught_up < 100 {
            if is_reset {
                last_tick = last_tick.wrapping_add(1);
                cycle_counter += 1;
                
                if !was_reset {
                    drive.reset(&mut bridge);
                    bridge.iec_drive(pico1541_common::interface::IecBusState::default());
                    was_reset = true;
                }
                bridge.sync(bridge.iec_host());
            } else {
                if was_reset {
                    drive.reset(&mut bridge);
                    was_reset = false;
                }
                let t1 = cortex_m::peripheral::SYST::get_current();
                let elapsed = drive.tick(&mut bridge);
                let t3 = cortex_m::peripheral::SYST::get_current();
                
                last_tick = last_tick.wrapping_add(elapsed);
                cycle_counter += elapsed;
                
                // SYST is a 24-bit down counter
                let total_cycles = t1.wrapping_sub(t3) & 0x00FFFFFF;
                cpu_cycles_acc += total_cycles as u64;
                ticks_acc += 1;
            }
            caught_up += 1;
        }

        // Update performance metrics every second
        if now.wrapping_sub(last_perf_update) >= 1_000_000 {
            let config = bridge.config;
            
            // Calculate average SYST cycles per 1541-tick
            let new_avg = if cycle_counter > 0 {
                (cpu_cycles_acc / cycle_counter as u64) as u32
            } else {
                0
            };
            
            config.emu_speed_hz.store(cycle_counter, Ordering::Relaxed);
            config.core0_avg_raw.store(new_avg, Ordering::Relaxed);

            if ticks_acc > 0 {
                info!("Profile: Total CPU={} (avg cycles per tick over {} ticks)", 
                    cpu_cycles_acc / ticks_acc, 
                    ticks_acc);
            }

            cpu_cycles_acc = 0;
            ticks_acc = 0;

            // We no longer auto-calibrate max_raw upwards here, 
            // because the baseline idle was calculated accurately at startup.
            
            cycle_counter = 0;
            idle_counter = 0;
            last_perf_update = now;
        } else {
            idle_counter = idle_counter.wrapping_add(1);
            cortex_m::asm::nop();
        }
    }
}
