#![no_std]
#![no_main]

extern crate alloc;

#[global_allocator]
static HEAP: embedded_alloc::LlffHeap = embedded_alloc::LlffHeap::empty();

mod driver;
mod flash_storage;

#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use embassy_rp::interrupt;

use core::sync::atomic::Ordering;
use defmt::{info, error};
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{Config, InterruptHandler};
use embassy_rp::peripherals::{I2C0, DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::spi::{Config as SpiConfig, Spi};
use embassy_rp::multicore::Stack;
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use pico1541_common::snooper::{SnooperConfig};
use pico1541_manager::tasks::{
    ActionTask, DiskTask, DisplayTask, DriveActivityTask, KeyboardTask, MainMenuTask, SnooperTask
};

#[cfg(has_mklittlefs)]
#[link_section = ".fs"]
#[used]
static FS_IMAGE: [u8; 512 * 1024] = *include_bytes!(concat!(env!("OUT_DIR"), "/fs.bin"));

// Local drivers
use crate::driver::display::{DisplayHardware, DisplayState, DisplayWrapper};
use crate::driver::leds::{StatusLeds, StatusLedsWrapper};
use crate::driver::input::KeyboardInput;
use crate::driver::speaker::PiezoSpeaker;
use crate::driver::blockdevsdcard::{SdCard, SdBlockIo};
use pico1541_manager::driver::file_storage::{FatStorage, StorageWrapper};
use crate::driver::indicator::{ActivityIndicator, IndicatorWrapper};

// Tasks and manager components
use pico1541_manager::{SETTINGS_CHANNEL, SettingsCommand};

use pico1541_common::SYSTEM_CLOCK_FREQ;
use pico1541_common::track_buffer::{DualCoreTrackBuffer, ManagerHandle};

mod storage_manager;
use storage_manager::*;
use static_cell::StaticCell;

pub mod littlefs_storage;
use littlefs_storage::LittleFsStorage;
use flash_storage::FlashStorage;

use pico1541_rp2040_emulation::emulation;


bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<DMA_CH0>, embassy_rp::dma::InterruptHandler<DMA_CH1>;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});

const DEFAULT_DRIVE_ID: u8 = 8;


static SNOOPER_CONFIG: SnooperConfig = SnooperConfig::new(DEFAULT_DRIVE_ID);

static CURRENT_DIR: Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)> = Mutex::new((heapless::String::new(), false));
static MOUNTED_DIR: Mutex<CriticalSectionRawMutex, (heapless::String<128>, bool)> = Mutex::new((heapless::String::new(), false));

static EXECUTOR_HIGH: embassy_executor::InterruptExecutor = embassy_executor::InterruptExecutor::new();

#[embassy_rp::interrupt]
unsafe fn TIMER_IRQ_3() {
    embassy_rp::pac::TIMER.intf().write(|w| w.set_alarm(3, false));
    EXECUTOR_HIGH.on_interrupt();
}


// Core 1 Stack
#[link_section = ".core1_stack"]
static mut CORE1_STACK: core::mem::MaybeUninit<Stack<1024>> = core::mem::MaybeUninit::uninit();

// ROM Buffer for dynamic loading
static mut ROM_BUFFER: [u8; 16384] = [0; 16384];



#[embassy_executor::main]
async fn main(spawner: Spawner) {
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
    embassy_rp::pac::VREG_AND_CHIP_RESET.vreg().write(|w| {
        w.set_vsel(0x0C);
        w.set_en(true);
    });

    let mut config = embassy_rp::config::Config::default();
    config.clocks = embassy_rp::clocks::ClockConfig::system_freq(SYSTEM_CLOCK_FREQ).unwrap();
    let p = embassy_rp::init(config);
    info!("Pico1541 Starting at {}MHz...", SYSTEM_CLOCK_FREQ / 1_000_000);

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
                    for entry in dir.flatten() {
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


    // Initialize the shared double track buffers
    static DUAL_CORE_BUFFER: DualCoreTrackBuffer = DualCoreTrackBuffer::new();
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
        rgb_red: p.PIN_26.into(),
        rgb_green: p.PIN_27.into(),
        rgb_blue: p.PIN_28.into(),
        spi1: p.SPI1.into(),
        clk: p.PIN_14.into(),
        mosi: p.PIN_15.into(),
        miso: p.PIN_12.into(),
        cs: p.PIN_13.into(),
        det: p.PIN_11.into(),
        kbd_row1: p.PIN_6.into(),
        kbd_row2: p.PIN_7.into(),
        kbd_col1: p.PIN_8.into(),
        kbd_col2: p.PIN_9.into(),
        pin25: p.PIN_25.into(),
    };

    // (track_addr unused)

    let emulator_hardware = emulation::EmulatorHardware {
        pin_16: p.PIN_16.into(),
        pin_17: p.PIN_17.into(),
        pin_18: p.PIN_18.into(),
        pin_19: p.PIN_19.into(),
        pin_20: p.PIN_20.into(),
        pin_21: p.PIN_21.into(),
        pin_22: p.PIN_22.into(),
        pin_3: p.PIN_3.into(),
        pin_0: p.PIN_0.into(),
        pin_1: p.PIN_1.into(),
    };

    let pio0 = embassy_rp::pio::Pio::new(p.PIO0, Irqs);

    info!("Starting Emulator on Core 1...");
    emulation::spawn_core1(
        p.CORE1.into(),
        stack,
        emulator_hardware,
        &SNOOPER_CONFIG,
        drive_id,
        pio0,
        emulator_handle,
        unsafe { &*core::ptr::addr_of!(ROM_BUFFER) },
    );

    // --- Run Manager on Core 0 ---
    spawner.spawn(settings_save_task(fs_storage_inst).unwrap());
    core0_init_task(spawner, core1_pins, &SNOOPER_CONFIG, rom_name, manager_handle).await;
}

#[embassy_executor::task]
async fn activity_run(mut task: DriveActivityTask<IndicatorWrapper<CriticalSectionRawMutex>>) {
    task.run().await;
}

#[embassy_executor::task]
async fn core0_perf_task(config: &'static pico1541_common::snooper::SnooperConfig) {
    let mut last_emu_ticks = config.emu_total_ticks.load(Ordering::Relaxed);
    let mut last_idle_loops = config.emu_total_idles.load(Ordering::Relaxed);
    
    // Start max idle at 1 so it quickly calibrates UP to the actual idle loops
    let mut max_idle_per_20ms = 1u32;
    let mut max_load = 0;
    
    let mut last_time = embassy_time::Instant::now();
    let mut ticks_since_last = 0;
    
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(20)).await;
        
        let current_emu_ticks = config.emu_total_ticks.load(Ordering::Relaxed);
        let current_idle_loops = config.emu_total_idles.load(Ordering::Relaxed);
        
        let diff_ticks = current_emu_ticks.wrapping_sub(last_emu_ticks);
        let diff_idles = current_idle_loops.wrapping_sub(last_idle_loops);
        
        last_emu_ticks = current_emu_ticks;
        last_idle_loops = current_idle_loops;
        
        ticks_since_last += diff_ticks;
        
        if diff_idles > max_idle_per_20ms {
            max_idle_per_20ms = diff_idles;
        }
        
        if max_idle_per_20ms > 0 {
            let idle_pct = if diff_idles > max_idle_per_20ms { 100 } else { (diff_idles * 100) / max_idle_per_20ms };
            let load_pct = 100 - idle_pct;
            
            // Reconstruct fake SYST cycles for compatibility with main_menu.rs
            let fake_busy_syst = (load_pct * (SYSTEM_CLOCK_FREQ / 50)) / 100;
            
            if fake_busy_syst > max_load {
                max_load = fake_busy_syst;
            }
        }
        
        let now = embassy_time::Instant::now();
        let elapsed_ms = now.duration_since(last_time).as_millis();
        
        // Update speed and reset stats roughly twice a second
        if elapsed_ms >= 500 {
            let speed_hz = ((ticks_since_last as u64 * 1000) / elapsed_ms) as u32;
            config.emu_speed_hz.store(speed_hz, Ordering::Relaxed);
            config.core0_avg_raw.store(max_load, Ordering::Relaxed);
            
            // FIXME! Remove this
            //defmt::info!("SPEED: {} Hz, MAX LOAD: {} (out of {})", speed_hz, max_load, (SYSTEM_CLOCK_FREQ / 50));

            ticks_since_last = 0;
            max_load = 0;
            last_time = now;
        }
    }
}

#[embassy_executor::task]
async fn snooper_run(mut task: SnooperTask) {
    task.run().await;
}




#[embassy_executor::task]
async fn vdisk_run(mut task: DiskTask<'static, MyStorage, MyDir, PlatformStorageManager>) {
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
#[allow(clippy::type_complexity)]
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

    pub rgb_red: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_26>,
    pub rgb_green: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_27>,
    pub rgb_blue: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_28>,

    pub spi1: embassy_rp::Peri<'static, embassy_rp::peripherals::SPI1>,
    pub clk: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_14>,
    pub mosi: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_15>,
    pub miso: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_12>,
    pub cs: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_13>,
    pub det: embassy_rp::Peri<'static, embassy_rp::peripherals::PIN_11>,

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

    let display_task = DisplayTask::new(display_wrapper.clone());

    // 7. Snooper Task
    let snooper_task = SnooperTask::new(8);

    // 8. Storage (Starts unmounted)
    static DISK_IMAGE: StaticCell<Mutex<CriticalSectionRawMutex, Option<MyDisk>>> = StaticCell::new();
    let disk_image_mutex = DISK_IMAGE.init(Mutex::new(None));

    let vdisk_task = DiskTask::new(
        &*disk_image_mutex,
        manager_handle,
        PlatformStorageManager { storage: StorageWrapper(sd_mutex), mounted_dir: &MOUNTED_DIR },
        &MOUNTED_DIR,
    );

    let main_menu_task = MainMenuTask::new(
        display_wrapper,
        StatusLedsWrapper(status_leds_mutex),
        IndicatorWrapper(indicator_mutex),
        StorageWrapper(sd_mutex),
        LittleFsStorage,
        &*disk_image_mutex,
        &CURRENT_DIR,
        &MOUNTED_DIR,
        config,
        rom_name,
    );

    spawner.spawn(menu_run(main_menu_task).unwrap());
    spawner.spawn(activity_run(activity_task).unwrap());
    spawner.spawn(keyboard_run(keyboard_task).unwrap());
    let action_task = ActionTask::new();
    spawner.spawn(action_run(action_task).unwrap());

    spawner.spawn(core0_perf_task(&SNOOPER_CONFIG).unwrap());
    spawner.spawn(display_run(display_task).unwrap());
    spawner.spawn(snooper_run(snooper_task).unwrap());
    use embassy_rp::interrupt::{InterruptExt, Priority};
    embassy_rp::interrupt::TIMER_IRQ_3.set_priority(Priority::P2);
    EXECUTOR_HIGH.start(embassy_rp::interrupt::TIMER_IRQ_3);
    let spawner_high = EXECUTOR_HIGH.spawner();
    spawner_high.spawn(vdisk_run(vdisk_task).unwrap());
    spawner.spawn(blinky(pins.pin25).unwrap());
    spawner.spawn(core1_load_run(config).unwrap());
}

#[embassy_executor::task]
async fn settings_save_task(mut flash_storage: FlashStorage<'static>) {
    loop {
        let cmd = SETTINGS_CHANNEL.receive().await;
        
        let mut config = littlefs_rust::Config::new(4096, 128);
        config.read_size = 256;
        config.prog_size = 256;
        config.cache_size = 256;
        config.lookahead_size = 16;
        
        match cmd {
                SettingsCommand::SaveDriveId(id) => {
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
                SettingsCommand::SaveBootRom(name) => {
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
                SettingsCommand::DeleteRom(name) => {
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
                SettingsCommand::InstallRomStart { filename, size: _, offset: _ } => {
                    match littlefs_rust::Filesystem::mount(flash_storage, config) {
                        Ok(fs) => {
                            let _ = fs.mkdir("/roms");
                            let mut full_path = heapless::String::<128>::new();
                            let _ = full_path.push_str("/roms/");
                            let _ = full_path.push_str(filename.as_str());
                            
                            if let Ok(file) = fs.open(full_path.as_str(), littlefs_rust::OpenFlags::WRITE | littlefs_rust::OpenFlags::CREATE | littlefs_rust::OpenFlags::TRUNC) {
                                info!("Opened {} for writing", full_path.as_str());
                                loop {
                                    let inner_cmd = SETTINGS_CHANNEL.receive().await;
                                    match inner_cmd {
                                        SettingsCommand::InstallRomChunk(data) => {
                                            let _ = file.write(&data);
                                        }
                                        SettingsCommand::InstallRomFinish => {
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
                SettingsCommand::LoadRomToMemory(name) => {
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


