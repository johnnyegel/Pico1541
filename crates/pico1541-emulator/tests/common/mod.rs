#![allow(dead_code)]
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;
use pico1541_emulator::snooper::IecSnooper;
use pico1541_common::formats::{AnyDiskImage, DiskImage, TrackLayout, G64Image};
use pico1541_common::storage::DummySource;
use futures::executor::block_on;
use log;
use racc6502::cpu::DebugHook;

const ROM_BYTES: &[u8] = include_bytes!("../../../../roms/1541-II.251968-03.bin");

/// Timeout for spin-waiting on bus transitions (cycles).
pub const IEC_TIMEOUT: u32 = 20_000_000;

/// Standard IEC Timings (in CPU cycles @ 1MHz)
pub const T_SETTLE: u32 = 100;  // ~100us (settle time)
pub const T_PHASE:  u32 = 60;   // ~60us (C64 minimum rate)
pub const T_EOI:    u32 = 200;  // >200us

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
    pub last_drv_data: bool, 
    pub last_drv_clk: bool,
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
            last_drv_data: false,
            last_drv_clk: false,
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.host_data = false;
        self.host_clk = false;
        self.host_atn = false;
        self.snooper = IecSnooper::new(self.device_id);
        self.sync_hook = None;
        self.last_drv_data = false;
        self.last_drv_clk = false;
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

    pub fn wait_bus_data(&mut self, drive: &mut Emulator<Self>, want_asserted: bool) {
        for _ in 0..IEC_TIMEOUT {
            drive.tick(self);
            self.sync();
            let bus_data = self.host_data || self.last_drv_data || self.snooper.should_drive_data();
            if bus_data == want_asserted { return; }
        }
        panic!("IEC timeout: DATA != {} (PC=${:04X}, host_data={}, drv_data={})", 
               want_asserted, drive.pc(), self.host_data, self.last_drv_data);
    }

    pub fn wait_bus_clk(&mut self, drive: &mut Emulator<Self>, want_asserted: bool) {
        for _ in 0..IEC_TIMEOUT {
            drive.tick(self);
            self.sync();
            let bus_clk = self.host_clk || self.last_drv_clk;
            if bus_clk == want_asserted { return; }
        }
        panic!("IEC timeout: CLK != {} (PC=${:04X}, host_clk={}, drv_clk={})", 
               want_asserted, drive.pc(), self.host_clk, self.last_drv_clk);
    }

    pub fn atn_start(&mut self, drive: &mut Emulator<Self>) {
        log::info!("[Test] ATN PULL");
        self.set_data(false); // Host releases DATA
        self.set_clk(false);  // Host releases CLK
        self.set_atn(true);   // Host pulls ATN LOW (asserted)

        // 1. Wait for drive to ACK by pulling DATA LOW (asserted)
        self.wait_bus_data(drive, true);

        // 2. Wait for drive to release DATA (Ready to accept command)
        self.wait_bus_data(drive, false);

        self.ticks(drive, T_PHASE);
        self.set_clk(true); // Host pulls CLK LOW (asserted)
    }


    pub fn atn_end(&mut self, drive: &mut Emulator<Self>) {
        log::info!("[Test] ATN RELEASE");
        self.ticks(drive, T_PHASE);
        self.set_atn(false);
    }

    pub fn iec_end(&mut self, drive: &mut Emulator<Self>) {
        self.ticks(drive, T_PHASE);
        self.set_atn(false);
        self.set_clk(false);
        self.set_data(false);
        if self.last_drv_data || self.host_data {
            self.wait_bus_data(drive, false);
        }
        self.ticks(drive, T_PHASE);
    }

    pub fn send_byte(&mut self, drive: &mut Emulator<Self>, byte: u8, eoi: bool) { 
        log::info!("[Test] Sending Byte: 0x{:02X} (EOI={})", byte, eoi);
        // Wait for ready-to-accept is already done in atn_start or previous byte loop
        self.set_clk(false); // Host releases CLK
        
        if eoi {
            self.wait_bus_data(drive, true); // Drive ACK EOI pull
            self.ticks(drive, T_EOI);
            self.set_clk(true);
            self.wait_bus_data(drive, false); // Drive ACK EOI release
            self.set_clk(false);
        } else {
            self.ticks(drive, 20);
        }

        self.ticks(drive, T_PHASE);
        self.set_clk(true); // Start bits

        for bit in 0..8u8 {
            let bit_val = (byte >> bit) & 1 != 0;
            self.set_data(!bit_val); // Logical 1 = Released (false)
            self.ticks(drive, T_PHASE);
            self.set_clk(false);
            self.ticks(drive, T_PHASE);
            self.set_clk(true);
        }
        self.set_data(false);
        self.ticks(drive, T_PHASE);
    }

    pub fn try_turnaround(&mut self, drive: &mut Emulator<Self>) -> bool {
        // 1. Release our pull on both lines.
        self.set_clk(false);
        self.set_data(false);

        // 2. Drive pulls CLK LOW (true) to acknowledge end of command phase.
        self.wait_bus_clk(drive, true);

        // 3. Drive then releases CLK (false) and pulls DATA LOW (true) to start talk.
        self.wait_bus_clk(drive, false);
        self.wait_bus_data(drive, true);
        true
    }
    pub fn turnaround(&mut self, drive: &mut Emulator<Self>) {
        if !self.try_turnaround(drive) {
            panic!("IEC turnaround timeout (PC=${:04X})", drive.pc());
        }
    }

    pub fn untalk(&mut self, drive: &mut Emulator<Self>) {
        self.atn_start(drive);
        self.send_byte(drive, 0x5F, false); // UNTALK
        self.atn_end(drive);
        self.iec_end(drive);
    }

    pub fn close_channel(&mut self, drive: &mut Emulator<Self>, channel: u8) {
        self.atn_start(drive);
        self.send_byte(drive, 0x20 | self.device_id, false);   // LISTEN device
        self.send_byte(drive, 0xE0 | (channel & 0x0F), false); // CLOSE channel
        self.send_byte(drive, 0x3F, false);              // UNLISTEN
        self.atn_end(drive);
        self.iec_end(drive);
    }

    pub fn receive_byte(&mut self, drive: &mut Emulator<Self>) -> (u8, bool) {
        self.wait_bus_clk(drive, true); // Wait for talker pull
        self.set_data(false); // Host ready
        let mut ack_ticks = 0;
        while (self.host_clk || self.last_drv_clk) && ack_ticks < 500 {
            drive.tick(self);
            self.sync();
            ack_ticks += 1;
        }
        let is_eoi = if !(self.host_clk || self.last_drv_clk) {
            false
        } else {
            self.set_data(true); // EOI ACK
            self.wait_bus_clk(drive, false);
            self.set_data(false);
            true
        };
        self.ticks(drive, T_PHASE);
        let mut byte_val = 0u8;
        for bit in 0..8u8 {
            self.wait_bus_clk(drive, true);
            if !(self.host_data || self.last_drv_data || self.snooper.should_drive_data()) {
                byte_val |= 1 << bit; // Phys HIGH = logical 1
            }
            self.wait_bus_clk(drive, false);
        }
        self.set_data(true);
        self.ticks(drive, T_SETTLE);
        (byte_val, is_eoi)
    }

    pub fn receive_all(&mut self, drive: &mut Emulator<Self>) -> Vec<u8> {
        let mut data = Vec::new();
        loop {
            let (byte, is_eoi) = self.receive_byte(drive);
            data.push(byte);
            if is_eoi { break; }
        }
        data
    }

    pub fn read_status(&mut self, drive: &mut Emulator<Self>) -> String {
        self.atn_start(drive);
        self.send_byte(drive, 0x40 | self.device_id, false); 
        self.send_byte(drive, 0x6F, false); 
        self.atn_end(drive);
        self.turnaround(drive);
        let bytes = self.receive_all(drive);
        self.iec_end(drive);
        String::from_utf8_lossy(&bytes).trim_end_matches('\r').to_string()
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
    fn drive_data(&mut self, v: bool) { self.last_drv_data = v; }
    fn drive_clk(&mut self, v: bool)  { self.last_drv_clk = v; }
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
        let b_clk = self.host_clk || self.last_drv_clk;
        let b_data = self.host_data || self.last_drv_data || self.snooper.should_drive_data();
        if let Some(cmd_data) = self.snooper.tick(h_atn, b_clk, b_data) {
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
