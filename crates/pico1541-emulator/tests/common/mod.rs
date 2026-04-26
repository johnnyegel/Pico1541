#![allow(dead_code)]
use pico1541_emulator::interface::{ActiveTrack, DriveEvent, HostInterface, IecBus};
use pico1541_emulator::Emulator;

use log;

use racc6502::cpu::DebugHook;

//const ROM_BYTES: &[u8] = include_bytes!("roms/1541-II.251968-03.bin");
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


use pico1541_emulator::snooper::IecSnooper;

/// Shared test double for HostInterface.
pub struct TestBridge {
    pub track: ActiveTrack,
    pub events: Vec<DriveEvent>,
    pub host_data: bool,
    pub host_clk: bool,
    pub host_atn: bool,
    pub bus_data: bool,
    pub bus_clk: bool,
    pub snooper: IecSnooper,
    pub device_id: u8,
    last_drv_data: bool,
    last_drv_clk: bool,
}

impl TestBridge {
    pub fn new(device_id: u8) -> Self {
        Self {
            track: ActiveTrack::new(),
            events: Vec::new(),
            host_data: false,
            host_clk: false,
            host_atn: false,
            bus_data: false,
            bus_clk: false,
            snooper: IecSnooper::new(device_id),
            device_id,
            last_drv_data: false,
            last_drv_clk: false,
        }
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.host_data = false;
        self.host_clk = false;
        self.host_atn = false;
        self.bus_data = false;
        self.bus_clk = false;
        self.snooper = IecSnooper::new(self.device_id);
        self.last_drv_data = false;
        self.last_drv_clk = false;
    }

    pub fn set_atn(&mut self, v: bool)  { self.host_atn = v; }
    pub fn set_clk(&mut self, v: bool)  { self.host_clk = v; }
    pub fn set_data(&mut self, v: bool) { self.host_data = v; }

    pub fn update_bus(&mut self) {
        let drive_data_val = if self.is_hijacked() { false } else { self.last_drv_data };
        let drive_clk_val = if self.is_hijacked() { false } else { self.last_drv_clk };

        self.bus_data = drive_data_val || self.snooper.should_drive_data();
        self.bus_clk = drive_clk_val;
    }

    pub fn ticks(&mut self, drive: &mut Emulator<TestBridge>, n: u32) {
        for _ in 0..n { 
            drive.tick(self); 
            self.sync();
        }
    }

    pub fn wait_bus_data(&mut self, drive: &mut Emulator<Self>, want: bool) { 
        for _ in 0..IEC_TIMEOUT {
            drive.tick(self);
            self.sync();
            if self.bus_data == want { return; }
        }
        panic!("IEC timeout: DATA != {} (PC=${:04X}, host_data={})", want, drive.pc(), self.host_data);
    }

    pub fn wait_bus_clk(&mut self, drive: &mut Emulator<Self>, want: bool) {
        for _ in 0..IEC_TIMEOUT {
            drive.tick(self);
            self.sync();
            if self.bus_clk == want { return; }
        }
        panic!("IEC timeout: CLK != {} (PC=${:04X}, host_clk={})", want, drive.pc(), self.host_clk);
    }

    pub fn atn_start(&mut self, drive: &mut Emulator<Self>) {
        log::info!("[Test] ATN PULL");
        self.set_atn(true);
        self.ticks(drive, T_PHASE);
        self.set_clk(true);
        self.wait_bus_data(drive, true);
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
        if self.bus_data {
            self.wait_bus_data(drive, false);
        }
        self.ticks(drive, T_PHASE);
    }

    pub fn send_byte(&mut self, drive: &mut Emulator<Self>, byte: u8, eoi: bool) { 
        log::info!("[Test] Sending Byte: 0x{:02X} (EOI={})", byte, eoi);
        if !self.host_clk {
            panic!("send_byte called with CLK not asserted! PC=${:04X}", drive.pc());
        }

        self.set_clk(false); 
        
        if eoi {
            if self.is_hijacked() {
                self.ticks(drive, T_EOI + 50);
                self.set_clk(true);
                self.set_clk(false);
                self.ticks(drive, 20);
            } else {
                // Normal EOI handshake with 1541 ROM
                self.wait_bus_data(drive, false); // Wait for Ready
                self.wait_bus_data(drive, true);  // Wait for EOI acknowledge pull
                
                self.set_clk(true);
                self.wait_bus_data(drive, true);
                self.set_clk(false);
                self.wait_bus_data(drive, false); // Wait for EOI acknowledge release
            }
        } else {
            if self.is_hijacked() {
                self.ticks(drive, 30);
            } else {
                self.wait_bus_data(drive, false);
            }
        }

        self.ticks(drive, T_PHASE);
        self.set_clk(true);

        for bit in 0..8u8 {
            let bit_val = (byte >> bit) & 1 != 0;
            self.set_data(!bit_val); 
            self.ticks(drive, T_PHASE);
            self.set_clk(false);
            self.ticks(drive, T_PHASE);
            self.set_clk(true);
        }

        self.set_data(false);
        self.wait_bus_data(drive, true);
    }



    pub fn try_turnaround(&mut self, drive: &mut Emulator<Self>) -> bool {
        self.set_clk(false);
        self.set_data(true);
        self.wait_bus_data(drive, false);

        let mut cycles = 0u32;
        while !self.bus_clk && cycles < IEC_TIMEOUT {
            drive.tick(self);
            self.sync();
            cycles += 1;
        }
        if !self.bus_clk {
            return false;
        }
        true
    }

    pub fn turnaround(&mut self, drive: &mut Emulator<Self>) {
        if !self.try_turnaround(drive) {
            panic!("IEC turnaround timeout: drive did not assert CLK (PC=${:04X})", drive.pc());
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

    pub fn read_status(&mut self, drive: &mut Emulator<Self>) -> String {
        self.atn_start(drive);
        self.send_byte(drive, 0x40 | self.device_id, false); // TALK device
        self.send_byte(drive, 0x6F, false); // SEC 15
        self.atn_end(drive);
        self.turnaround(drive);
        let bytes = self.receive_all(drive);
        self.iec_end(drive);
        String::from_utf8_lossy(&bytes)
            .trim_end_matches('\r')
            .to_string()
    }

    pub fn receive_byte(&mut self, drive: &mut Emulator<Self>) -> (u8, bool) {
        self.wait_bus_clk(drive, false);
        self.set_data(false);
        let mut ack_ticks = 0;
        while !self.bus_clk && ack_ticks < 500 {
            drive.tick(self);
            self.sync();
            ack_ticks += 1;
        }
        let is_eoi = if self.bus_clk {
            false
        } else {
            self.set_data(true);
            self.wait_bus_clk(drive, true);
            self.set_data(false);
            true
        };
        self.ticks(drive, T_PHASE);
        let mut byte_val = 0u8;
        for bit in 0..8u8 {
            self.wait_bus_clk(drive, false);
            if !self.bus_data {
                byte_val |= 1 << bit;
            }
            self.wait_bus_clk(drive, true);
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

}
impl IecBus for TestBridge {
    fn read_data(&self) -> bool { self.host_data || self.bus_data }
    fn read_clk(&self)  -> bool { self.host_clk || self.bus_clk }
    fn read_atn(&self)  -> bool { self.host_atn }
    fn drive_data(&mut self, v: bool) { 
        self.last_drv_data = v;
        self.update_bus();
    }
    fn drive_clk(&mut self, v: bool)  { 
        self.last_drv_clk = v;
        self.update_bus();
    }
}


impl HostInterface for TestBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.track }
    fn send_event(&mut self, event: DriveEvent) { self.events.push(event); }

    fn is_hijacked(&self) -> bool {
        self.snooper.hijack_active
    }

    fn pop_discard_request(&mut self) -> bool {
        let req = self.snooper.discard_request;
        self.snooper.discard_request = false;
        req
    }

    fn update_iec_addresses(&mut self, listen: u8, talk: u8) {
        self.snooper.set_iec_addresses(listen, talk);
    }

    fn sync(&mut self) {
        let atn = self.host_atn;
        let clk = self.host_clk || self.bus_clk; 
        let data = self.host_data || self.bus_data;

        if let Some(cmd_data) = self.snooper.tick(atn, clk, data) {
            let len = self.snooper.buffer_len() as u8;
            self.send_event(DriveEvent::IecCommand {
                data: cmd_data,
                len,
            });
        }
        
        self.update_bus();
    }
}
