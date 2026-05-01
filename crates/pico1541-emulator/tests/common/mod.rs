#![allow(dead_code)]
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;
use pico1541_emulator::snooper::IecSnooper;
use pico1541_common::formats::{AnyDiskImage, DiskImage, TrackLayout, G64Image};
use pico1541_common::storage::DummySource;
use pico1541_common::iec_com::{IecBus as IecHostBus, IecTransaction, IEC_TALK, IEC_UNTALK};
use pollster::block_on;
use log;
use racc6502::cpu::DebugHook;

const ROM_BYTES: &[u8] = include_bytes!("../../../../roms/1541-II.251968-03.bin");


/// 1541-II ROM idle loop address range.
pub const IDLE_LOW:  u16 = 0xEBFF;
pub const IDLE_HIGH: u16 = 0xEC9B;

pub fn setup_drive(reset: bool, device_id: u8, debug_hook: Option<DebugHook>) -> (Emulator<TestBridge>, TestBridge) {
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

    log::info!("Drive setup complete (device ID {}, reset={})", device_id, reset);

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

    panic!("Drive did not reach idle loop after reset (PC=${:04X})", drive.pc());
}

pub struct DiskRunner<S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static> {
    pub image: AnyDiskImage<S, DummySource>,
    pub last_half_track: u8,
}

impl<S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static> DiskRunner<S> {
    pub fn sync(&mut self, track: &mut ActiveTrack) {
        track.disk_inserted = true;
        if track.half_track != self.last_half_track {
            log::info!("[DiskRunner] Loading track data for half-track {}", track.half_track);
            let mut gcr_data = [0u8; pico1541_emulator::interface::MAX_TRACK_BYTES];
            let layout = block_on(self.image.read_track(track.half_track, &mut gcr_data)).unwrap_or(TrackLayout::Empty);
            
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
    pub snooper: IecSnooper,
    pub device_id: u8,
    pub sync_hook: Option<Box<dyn FnMut(&mut ActiveTrack)>>,
    pub drive_data: bool, 
    pub drive_clk: bool,
}

impl TestBridge {
    pub fn new(device_id: u8) -> Self {
        Self {
            track: ActiveTrack::new(),
            events: Vec::new(),
            host_data: false,
            host_clk: false,
            host_atn: false,
            snooper: IecSnooper::new(device_id),
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
        self.snooper = IecSnooper::new(self.device_id);
        self.sync_hook = None;
        self.drive_data = false;
        self.drive_clk = false;
    }

    pub fn set_atn(&mut self, v: bool)  { self.host_atn = v; }
    pub fn set_clk(&mut self, v: bool)  { self.host_clk = v; }
    pub fn set_data(&mut self, v: bool) { self.host_data = v; }

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
        let mut runner = DiskRunner { image: AnyDiskImage::G64(img), last_half_track: 255 };
        self.sync_hook = Some(Box::new(move |track| { runner.sync(track); }));
    }
}

impl IecBus for TestBridge {
    fn read_data(&self) -> bool { self.host_data }
    fn read_clk(&self)  -> bool { self.host_clk }
    fn read_atn(&self)  -> bool { self.host_atn }
    fn drive_data(&mut self, v: bool) { self.drive_data = v; }
    fn drive_clk(&mut self, v: bool)  { self.drive_clk = v; }
}

impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, event: DriveEvent) { self.events.push(event); }
    fn is_hijacked(&self) -> bool { self.snooper.hijack_active }
    fn pop_discard_request(&mut self) -> bool {
        let req = self.snooper.discard_request;
        self.snooper.discard_request = false;
        req
    }
    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.snooper.set_iec_addresses(listen, talk);
    }
    fn sync(&mut self) {
        let h_atn = self.host_atn;
        // The snooper only needs to observe incoming host lines to intercept
        // commands directed from the host to the drive.
        let h_clk = self.host_clk;
        let h_data = self.host_data;
        
        if let Some(cmd_data) = self.snooper.tick(h_atn, h_clk, h_data) {
            let len = self.snooper.buffer_len() as u8;
            self.send_event(DriveEvent::IecCommand { data: cmd_data, len });
        }
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
    fn set_atn(&mut self, assert: bool) { self.bridge.set_atn(assert); }
    fn set_clk(&mut self, assert: bool) { self.bridge.set_clk(assert); }
    fn set_data(&mut self, assert: bool) { self.bridge.set_data(assert); }
    
    // Checking for drive assertions simply involves observing the drive lines.
    fn get_clk(&self) -> bool { self.bridge.drive_clk }
    fn get_data(&self) -> bool { self.bridge.drive_data }

    fn trace_state(&self, tag: &str) -> () {
        log::trace!("[{}]: PC=${:04X}, DATA={}, CLK={}", 
            tag, self.drive.pc(), self.bridge.drive_data, self.bridge.drive_clk);
    }

    async fn delay_us(&mut self, us: u32) {
        for _ in 0..us {
            self.drive.tick(self.bridge);
            self.bridge.sync();
        }
    }
}


pub fn iec_com<'a, 'b>(bus: &'a mut IecTestBridge<'b>) -> IecTransaction<'a, IecTestBridge<'b>> {
    IecTransaction::new(bus)
}

pub fn read_status(drive: &mut Emulator<TestBridge>, bridge: &mut TestBridge) -> String {
    let device_id = bridge.device_id;
    let received = block_on(async {
        let mut bus = IecTestBridge { drive, bridge };
        let com = iec_com(&mut bus);
        let mut com = com.atn_assert().await?
            .send(IEC_TALK | device_id).await?
            .send(0x6F).await?
            .atn_release().await?;
        
        let mut bytes = Vec::new();
        loop {
            let mut byte = 0u8;
            let mut eoi = false;

            com = com.recv(&mut byte, &mut eoi).await?;
            bytes.push(byte);
            if eoi { break; }
        }
        
        // Finalize the communication and return the bus to a known state before ticking the drive further.
        com.atn_assert().await?
            .send(IEC_UNTALK).await?
            .atn_release().await?
            .done().await?;

        Ok::<Vec<u8>, pico1541_common::iec_com::IecError>(bytes)
    }).unwrap();
    
    String::from_utf8_lossy(&received).trim_end_matches('\r').to_string()
}
