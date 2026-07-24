#![allow(dead_code)]
use pico1541_common::snooper::IecBusState;
#[allow(dead_code)]
#[allow(unused_imports)]
use pico1541_common::context::ViaHardware;
use log;
use pico1541_common::formats::{AnyDiskImage, DiskImage, G64Image};
use pico1541_common::track_buffer::TrackLayout;
use pico1541_common::iec_com::IecComHostBus;
use pico1541_common::storage::DummySource;
use pico1541_common::snooper::{IecSnooper, IecSnoopByteHandler};
use pico1541_common::interface::{DriveEvent, HostInterface};
use pico1541_common::magnetic_surface::SoftwareMagneticSurface;
use pico1541_emulator::Emulator;
use pollster::block_on;

static ROM_BYTES: [u8; 16384] = *include_bytes!("../../../../flash-fs/roms/1541-II.251968-03.bin");

/// 1541-II ROM idle loop address range.
pub const IDLE_LOW: u16 = 0xEBFF;
pub const IDLE_HIGH: u16 = 0xEC9B;

pub fn setup_drive(
    reset: bool,
    device_id: u8,
) -> (Emulator<TestBridge, SoftwareMagneticSurface>, TestBridge) {
    let _ = env_logger::builder()
        .is_test(true)
        .parse_default_env()
        .try_init();

    let buffer = Box::leak(Box::new(pico1541_common::track_buffer::DualCoreTrackBuffer::new()));
    let (_manager, emulator) = buffer.split();

    let surface = SoftwareMagneticSurface::new(emulator);
    let mut drive = Emulator::<TestBridge, SoftwareMagneticSurface>::new(&ROM_BYTES, device_id, surface);
    let mut bridge = TestBridge::new(device_id, buffer);

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

pub fn reset_drive(drive: &mut Emulator<TestBridge, SoftwareMagneticSurface>, bridge: &mut TestBridge) {
    bridge.reset();
    drive.reset(bridge);

    // Boot: Wait for idle loop to be stable
    let mut stable_cycles = 0;
    for _ in 0..10_000_000 {
        drive.tick(bridge);
        if drive.pc() >= IDLE_LOW && drive.pc() <= IDLE_HIGH {
            stable_cycles += 1;
            if stable_cycles > 100 {
                return;
            }
        } else {
            stable_cycles = 0;
        }
    }

    panic!(
        "Drive did not reach idle loop after reset (PC=${:04X})",
        drive.pc()
    );
}

pub struct DiskRunner<S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static, D: pico1541_common::storage::DirectorySource = DummySource> {
    pub image: AnyDiskImage<S, D>,
    pub last_half_track: u8,
}

impl<S, D> DiskRunner<S, D> 
where 
    S: embedded_io_async::Read + embedded_io_async::Seek + Send + 'static,
    D: pico1541_common::storage::DirectorySource
{
    pub fn sync(&mut self, buffer: &'static pico1541_common::track_buffer::DualCoreTrackBuffer) {
        let manager = buffer.split().0;
        let track_ht = manager.get_active_half_track();
        if track_ht != self.last_half_track {
            log::info!(
                "[DiskRunner] Loading track data for half-track {}",
                track_ht
            );
            
            manager.duplicate_active_to_inactive();
            let inactive = manager.get_dormant_buffer_mut();
            
            let layout = block_on(self.image.read_track(track_ht, &mut inactive.data))
                .unwrap_or((TrackLayout::Void, 0, None));

            manager.commit_loaded_track(track_ht, layout.0, layout.1, layout.2);

            self.last_half_track = track_ht;
        }
    }
}

pub struct TestBridge {
    pub buffer: &'static pico1541_common::track_buffer::DualCoreTrackBuffer,
    pub events: Vec<DriveEvent>,
    pub snooper: IecSnooper,
    pub snoop_handler: IecSnoopByteHandler,
    pub device_id: u8,
    pub discard_request: bool,
    pub total_cycles: u64,
    pub host_iec: IecBusState,
    pub drive_iec: IecBusState,
    pub sync_hook: Option<Box<dyn FnMut(&'static pico1541_common::track_buffer::DualCoreTrackBuffer)>>,
}

impl TestBridge {
    pub fn new(device_id: u8, buffer: &'static pico1541_common::track_buffer::DualCoreTrackBuffer) -> Self {
        Self {
            buffer,
            events: Vec::new(),
            snooper: IecSnooper::new(device_id),
            snoop_handler: IecSnoopByteHandler::new(),
            device_id,
            discard_request: false,
            total_cycles: 0,
            host_iec: IecBusState::default(),
            drive_iec: IecBusState::default(),
            sync_hook: None,
        }
    }

    pub fn reset(&mut self) {
        let manager = self.buffer.split().0;
        let inactive = manager.get_dormant_buffer_mut();
        inactive.track_id = 36;
        manager.commit_dormant_buffer();
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

    pub fn is_idle(&self, drive: &Emulator<TestBridge, SoftwareMagneticSurface>) -> bool {
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

    pub fn ticks(&mut self, drive: &mut Emulator<TestBridge, SoftwareMagneticSurface>, n: u32) {
        let start = drive.cycles;
        while drive.cycles.wrapping_sub(start) < (n as u32) {
            drive.tick(self);
        }
    }

    pub fn mount_g64(&mut self, path: &str) {
        use pico1541_common::storage::{MemoryStorage, DummySource};
        use std::fs;
        let data = fs::read(path).expect("Failed to read G64");
        let data_ref: &'static [u8] = Box::leak(data.into_boxed_slice());
        let img = futures::executor::block_on(G64Image::from_storage(MemoryStorage::new(data_ref))).unwrap();
        let mut runner = DiskRunner {
            image: AnyDiskImage::<MemoryStorage<'static>, DummySource>::G64(img),
            last_half_track: 255,
        };
        self.sync_hook = Some(Box::new(move |buffer| {
            runner.sync(buffer);
        }));
    }
}


impl ViaHardware for TestBridge {
    fn read_iec_port(&mut self) -> u8 {
        let mut val = 0x00;
        let mut combined = self.host_iec;
        if self.drive_iec.get_data() { combined.set_data(true); }
        if self.drive_iec.get_clk() { combined.set_clk(true); }
        if self.drive_iec.get_atn() { combined.set_atn(true); }

        if combined.get_data() { val |= Self::VIA1_IEC_MASK_DATA_RX; } // 1 = asserted
        if combined.get_clk() { val |= Self::VIA1_IEC_MASK_CLK_RX; } // 1 = asserted
        if combined.get_atn() { val |= Self::VIA1_IEC_MASK_ATN_RX; } // 1 = asserted
        val
    }

    fn write_iec_port(&mut self, via_port_b: u8) {
        let assert_data = (via_port_b & Self::VIA1_IEC_MASK_DATA_TX) != 0;
        let assert_clk = (via_port_b & Self::VIA1_IEC_MASK_CLK_TX) != 0;
        self.drive_iec.set_data(assert_data);
        self.drive_iec.set_clk(assert_clk);
    }
}
impl HostInterface for TestBridge {
    fn send_event(&mut self, event: DriveEvent) {
        self.events.push(event);
        log::info!("[DriveEvent] {:?}", event);
    }
    
    fn track_changed(&mut self, half_track: u8) {
        log::info!("Head stepped to {}", half_track);
        let manager = self.buffer.split().0;
        manager.duplicate_active_to_inactive();
        let inactive = manager.get_dormant_buffer_mut();
        inactive.track_id = half_track;
        manager.commit_dormant_buffer();
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

    fn step(&mut self) {
        let mut events = Vec::new();
        // Update the snooper using ONLY host physical state (Mandate that it doesn't see drive state)
        self.snooper.update(self.host_iec, |event| {
             events.push(event);
        });

        for event in events {
            self.events.push(event);
            let mut extra_events = Vec::new();
            self.snoop_handler.handle(event, &mut |e| extra_events.push(e));
            
            for e in extra_events {
                self.events.push(e);
                // Also check if any extra event triggers hijack/discard
                if let DriveEvent::IecCommand { data, len } = e {
                     let cmd = String::from_utf8_lossy(&data[..len as usize]);
                     log::info!("[SnooperCommand] {:?}", cmd);
                }
            }
            
            // Once "CD:" is detected, we "yank the cable" by hijacking the bus
            // and requesting a discard of the command buffer.
            if self.snoop_handler.is_hijacked() && !self.discard_request {
                log::info!("[SnoopHandler] Hijack requested!");
                self.discard_request = true;
            }
        }
        
        self.total_cycles += 1;
        if let Some(mut hook) = self.sync_hook.take() {
            hook(self.buffer);
            self.sync_hook = Some(hook);
        }
    }
}

pub struct IecTestBridge<'a> {
    pub drive: &'a mut Emulator<TestBridge, SoftwareMagneticSurface>,
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
        let start = self.drive.cycles;
        while self.drive.cycles.wrapping_sub(start) < (us as u32) {
            self.drive.tick(self.bridge);
        }
    }
}
