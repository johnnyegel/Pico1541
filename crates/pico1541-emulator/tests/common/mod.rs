#![allow(dead_code)]
use log;
use pico1541_common::formats::{AnyDiskImage, DiskImage, G64Image, TrackLayout};
use pico1541_common::iec_com::IecBus as IecHostBus;
use pico1541_common::storage::DummySource;
use pico1541_common::snooper::{IecBusByte, IecSnooper, SnooperConfig};
use pico1541_emulator::interceptor::BusInterceptor;
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;
use pollster::block_on;
use racc6502::cpu::DebugHook;

const ROM_BYTES: &[u8] = include_bytes!("../../../../roms/1541-II.251968-03.bin");

/// 1541-II ROM idle loop address range.
pub const IDLE_LOW: u16 = 0xEBFF;
pub const IDLE_HIGH: u16 = 0xEC9B;

pub fn setup_drive(
    reset: bool,
    device_id: u8,
    debug_hook: Option<DebugHook>,
) -> (Emulator<TestBridge>, TestBridge) {
    let _ = env_logger::builder()
        .is_test(true)
        .parse_default_env()
        .try_init();

    let mut rom = [0u8; 16384];
    rom.copy_from_slice(ROM_BYTES);

    let mut drive = Emulator::<TestBridge>::new(rom, device_id);
    let mut bridge = TestBridge::new(device_id);

    if let Some(hook) = debug_hook {
        drive.cpu.debug_hook = Some(hook);
    }

    log::info!(
        "Drive setup complete (device ID {}, reset={})",
        device_id,
        reset
    );

    if reset {
        reset_drive(&mut drive, &mut bridge);
    }

    (drive, bridge)
}

pub fn reset_drive(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) {
    bridge.reset();
    drive.reset();

    // Boot: Wait for idle loop
    for _ in 0..5_000_000 {
        drive.tick(bridge);
        bridge.sync();
        if drive.pc() >= IDLE_LOW && drive.pc() <= IDLE_HIGH {
            return;
        }
    }

    panic!(
        "Drive did not reach idle loop after reset (PC=${:04X})",
        drive.pc()
    );
}

pub struct DiskRunner<S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static> {
    pub image: AnyDiskImage<S, DummySource>,
    pub last_half_track: u8,
}

impl<S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static> DiskRunner<S> {
    pub fn sync(&mut self, track: &mut ActiveTrack) {
        track.disk_inserted = true;
        if track.half_track != self.last_half_track {
            log::info!(
                "[DiskRunner] Loading track data for half-track {}",
                track.half_track
            );
            let mut gcr_data = [0u8; pico1541_emulator::interface::MAX_TRACK_BYTES];
            let layout = block_on(self.image.read_track(track.half_track, &mut gcr_data))
                .unwrap_or(TrackLayout::Empty);

            match layout {
                TrackLayout::Accurate(bits) => {
                    track.gcr_data.copy_from_slice(&gcr_data);
                    track.bit_length = bits;
                    track.is_formatted = true;
                    track.is_accurate = true;
                }
                TrackLayout::Warp(bits) => {
                    track.gcr_data.copy_from_slice(&gcr_data);
                    track.bit_length = bits;
                    track.is_formatted = true;
                    track.is_accurate = false;
                }
                TrackLayout::Empty => {
                    track.bit_length = 0;
                    track.is_formatted = false;
                    track.is_accurate = false;
                }
            }
            self.last_half_track = track.half_track;
        }
    }
}

pub struct TestBridge {
    pub track: ActiveTrack,
    pub events: Vec<DriveEvent>,
    pub host_data: bool, // true = ASSERTED (LOW)
    pub host_clk: bool,  // true = ASSERTED (LOW)
    pub host_atn: bool,  // true = ASSERTED (LOW)
    pub interceptor: BusInterceptor,
    pub snooper: IecSnooper,
    pub config: &'static SnooperConfig,
    pub device_id: u8,
    pub sync_hook: Option<Box<dyn FnMut(&mut ActiveTrack)>>,
    pub drive_data: bool,
    pub drive_clk: bool,
}

impl TestBridge {
    pub fn new(device_id: u8) -> Self {
        let config = Box::leak(Box::new(SnooperConfig::new(device_id)));
        Self {
            track: ActiveTrack::new(),
            events: Vec::new(),
            host_data: false,
            host_clk: false,
            host_atn: false,
            interceptor: BusInterceptor::new(),
            snooper: IecSnooper::new(device_id, config),
            config,
            device_id,
            sync_hook: None,
            drive_data: false,
            drive_clk: false,
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.host_data = false;
        self.host_clk = false;
        self.host_atn = false;
        self.config.reset(self.device_id);
        self.interceptor = BusInterceptor::new();
        self.snooper = IecSnooper::new(self.device_id, self.config);
        self.sync_hook = None;
        self.drive_data = false;
        self.drive_clk = false;
    }

    pub fn set_atn(&mut self, v: bool) {
        self.host_atn = v;
    }
    pub fn set_clk(&mut self, v: bool) {
        self.host_clk = v;
    }
    pub fn set_data(&mut self, v: bool) {
        self.host_data = v;
    }

    pub fn ticks(&mut self, drive: &mut Emulator<TestBridge>, n: u32) {
        for _ in 0..n {
            drive.tick(self);
            self.sync();
        }
    }

    pub fn mount_g64(&mut self, path: &str) {
        use pico1541_common::storage::MemoryStorage;
        use std::fs;
        let data = fs::read(path).expect("Failed to read G64");
        let data_ref: &'static [u8] = Box::leak(data.into_boxed_slice());
        let img = G64Image::new(MemoryStorage::new(data_ref));
        let mut runner = DiskRunner {
            image: AnyDiskImage::G64(img),
            last_half_track: 255,
        };
        self.sync_hook = Some(Box::new(move |track| {
            runner.sync(track);
        }));
    }
}

impl IecBus for TestBridge {
    fn read_data(&self) -> bool {
        self.host_data || self.drive_data
    }
    fn read_clk(&self) -> bool {
        self.host_clk || self.drive_clk
    }
    fn read_atn(&self) -> bool {
        self.host_atn
    }
    fn drive_data(&mut self, assert: bool) {
        let should_drive = self.config.should_drive_data.load(std::sync::atomic::Ordering::Relaxed);
        self.drive_data = assert || should_drive;
    }
    fn drive_clk(&mut self, assert: bool) {
        self.drive_clk = assert;
    }
}

impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        &mut self.track
    }
    fn send_event(&mut self, event: DriveEvent) {
        self.events.push(event);
    }
    fn is_hijacked(&self) -> bool {
        self.config.hijack_active.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn pop_discard_request(&mut self) -> bool {
        self.config.discard_request.swap(false, std::sync::atomic::Ordering::Relaxed)
    }
    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.config.listen_addr.store(listen, std::sync::atomic::Ordering::Relaxed);
        self.config.talk_addr.store(talk, std::sync::atomic::Ordering::Relaxed);
    }
    fn on_bus_cycle(&mut self, pb: u8) {
        let state = IecBusByte::from_pb(pb);
        if let Some((state, delta)) = self.interceptor.tick(state) {
            if let Some(cmd_data) = self.snooper.tick(state, delta) {
                let len = self.snooper.buffer_len() as u8;
                self.send_event(DriveEvent::IecCommand {
                    data: cmd_data,
                    len,
                });
            }
        }
    }
    fn sync(&mut self) {
        if let Some(mut hook) = self.sync_hook.take() {
            let track = self.get_active_track();
            hook(track);
            self.sync_hook = Some(hook);
        }
    }
}

pub struct IecTestBridge<'a> {
    pub drive: &'a mut Emulator<TestBridge>,
    pub bridge: &'a mut TestBridge,
}

impl<'a> IecHostBus for IecTestBridge<'a> {
    fn set_atn(&mut self, assert: bool) {
        self.bridge.set_atn(assert);
    }
    fn set_clk(&mut self, assert: bool) {
        self.bridge.set_clk(assert);
    }
    fn set_data(&mut self, assert: bool) {
        self.bridge.set_data(assert);
    }

    // Checking for drive assertions simply involves observing the drive lines.
    fn get_clk(&self) -> bool {
        self.bridge.drive_clk
    }
    fn get_data(&self) -> bool {
        self.bridge.drive_data
    }

    fn trace_state(&self, tag: &str) -> () {
        log::trace!(
            "[{}]: PC=${:04X}, DATA={}, CLK={}",
            tag,
            self.drive.pc(),
            self.bridge.drive_data,
            self.bridge.drive_clk
        );
    }

    async fn delay_us(&mut self, us: u32) {
        for _ in 0..us {
            self.drive.tick(self.bridge);
            self.bridge.sync();
        }
    }
}
