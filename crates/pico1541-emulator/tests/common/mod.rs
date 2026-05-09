#![allow(dead_code)]
use log;
use pico1541_common::formats::{AnyDiskImage, DiskImage, G64Image, TrackLayout};
use pico1541_common::iec_com::IecComHostBus;
use pico1541_common::interface::IecBusState;
use pico1541_common::storage::DummySource;
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler};
use pico1541_common::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
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
        if track.half_track != self.last_half_track {
            log::info!(
                "[DiskRunner] Loading track data for half-track {}",
                track.half_track
            );
            let mut gcr_data = [0u8; pico1541_common::interface::MAX_TRACK_BYTES];
            let layout = block_on(self.image.read_track(track.half_track, &mut gcr_data))
                .unwrap_or(TrackLayout::Void);

            track.layout = layout;

            if layout != TrackLayout::Void {
                track.gcr_data.copy_from_slice(&gcr_data);
            }

            self.last_half_track = track.half_track;
        }
    }
}

pub struct TestBridge {
    pub track: ActiveTrack,
    pub events: Vec<DriveEvent>,
    pub host_iec: IecBusState,
    pub drive_iec: IecBusState,
    pub snooper: IecSnooper,
    pub snoop_handler: IecSnoopByteHandler,
    pub device_id: u8,
    pub discard_request: bool,
    pub total_cycles: u64,
    pub sync_hook: Option<Box<dyn FnMut(&mut ActiveTrack)>>,
}

impl TestBridge {
    pub fn new(device_id: u8) -> Self {
        let mut track = ActiveTrack::new();
        track.half_track = 34; // INITIAL_HALF_TRACK

        Self {
            track,
            events: Vec::new(),
            host_iec: IecBusState::default(),
            drive_iec: IecBusState::default(),
            snooper: IecSnooper::new(device_id),
            snoop_handler: IecSnoopByteHandler::new(),
            device_id,
            discard_request: false,
            total_cycles: 0,
            sync_hook: None,
        }
    }

    pub fn reset(&mut self) {
        self.track.half_track = 34;
        self.events.clear();
        self.host_iec = IecBusState::default();
        self.drive_iec = IecBusState::default();
        self.snooper = IecSnooper::new(self.device_id);
        self.snoop_handler = IecSnoopByteHandler::new();
        self.discard_request = false;
        self.total_cycles = 0;
        self.sync_hook = None;
    }

    pub fn cycles(&self) -> u64 {
        self.total_cycles
    }

    pub fn is_idle(&self, drive: &Emulator<TestBridge>) -> bool {
        drive.pc() >= IDLE_LOW && drive.pc() <= IDLE_HIGH
    }

    pub fn set_atn(&mut self, v: bool) {
        self.host_iec.set_atn(v);
    }
    pub fn set_clk(&mut self, v: bool) {
        self.host_iec.set_clk(v);
    }
    pub fn set_data(&mut self, v: bool) {
        self.host_iec.set_data(v);
    }

    pub fn ticks(&mut self, drive: &mut Emulator<TestBridge>, n: u32) {
        for _ in 0..n {
            drive.tick(self);
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
    fn iec_host(&self) -> IecBusState {
        self.host_iec
    }
    
    fn iec_drive(&mut self, state: IecBusState) {
        self.drive_iec = state;
    }
}

impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack {
        &mut self.track
    }
    fn send_event(&mut self, event: DriveEvent) {
        self.events.push(event);
        log::info!("[DriveEvent] {:?}", event);
        match event {
            DriveEvent::HeadStepped { new_half_track } => {
                self.track.half_track = new_half_track;
            }
            _ => {}
        }
    }
    fn is_hijacked(&self) -> bool {
        self.snoop_handler.is_hijacked()
    }
    fn pop_discard_request(&mut self) -> bool {
        if self.discard_request {
            self.discard_request = false;
            true
        } else {
            false
        }
    }
    fn update_iec_addresses(&mut self, listen: u8, _talk: u8) {
        self.snooper.set_listen_addr(listen);
    }

    fn sync(&mut self) {
        self.total_cycles += 1;
        if let Some(mut hook) = self.sync_hook.take() {
            let track = self.get_active_track();
            hook(track);
            self.sync_hook = Some(hook);
        }

        // Update the snooper using combined physical state (Host + Drive)
        let combined_state = self.host_iec.combine(&self.drive_iec);
        let mut events = Vec::new();
        self.snooper.update(combined_state, |event| {
            events.push(event);
        });

        for event in events {
            self.events.push(event);
            self.snoop_handler.handle(event);
            
            // Once "CD:" is detected, we "yank the cable" by hijacking the bus
            // and requesting a discard of the command buffer.
            if self.snoop_handler.is_hijacked() && !self.discard_request {
                self.discard_request = true;
            }
        }
    }
}

pub struct IecTestBridge<'a> {
    pub drive: &'a mut Emulator<TestBridge>,
    pub bridge: &'a mut TestBridge,
}


impl<'a> IecComHostBus for IecTestBridge<'a> {
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
        self.bridge.drive_iec.get_clk()
    }
    fn get_data(&self) -> bool {
        self.bridge.drive_iec.get_data()
    }

    fn trace_state(&self, tag: &str) -> () {
        log::trace!(
            "[{}]: PC=${:04X}, IEC={}",
            tag,
            self.drive.pc(),
            self.bridge.drive_iec
        );
    }

    async fn delay_us(&mut self, us: u32) {
        for _ in 0..us {
            self.drive.tick(self.bridge);
        }
    }
}
