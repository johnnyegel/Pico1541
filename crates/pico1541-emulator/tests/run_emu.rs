#![cfg(any())]
use pico1541_common::formats::G64Image;
use pico1541_common::track_buffer::TrackBuffer;
use pico1541_common::magnetic_surface::{MagneticSurface, SoftwareMagneticSurface};
use pico1541_emulator::Emulator;
use pico1541_common::interface::HostInterface;
use std::fs::File;
use std::io::Read;

struct DummyHost;
impl pico1541_common::context::ViaHardware for DummyHost {}
impl HostInterface for DummyHost {
    fn set_activity_led(&mut self, _on: bool) {}
    fn debug_pin_0(&mut self, _on: bool) {}
    fn debug_pin_1(&mut self, _on: bool) {}
    fn debug_pin_2(&mut self, _on: bool) {}
    fn debug_pin_3(&mut self, _on: bool) {}
}

fn main() {
    let mut file = File::open("/home/johnny/git/dev/Pico1541/temp/Cauldron.g64").unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    let g64 = G64Image::parse(&buffer).unwrap();
    
    let mut track_buffer = TrackBuffer::new();
    let mut handle = track_buffer.get_emulator_handle();
    
    let track_data = g64.get_track(34).unwrap();
    handle.update_track_buffer(track_data);
    
    let mut surface = SoftwareMagneticSurface::new(handle);
    surface.start_reading(34, 0); // start reading track 34 (17)
    
    let mut rom = Vec::new();
    File::open("/home/johnny/git/dev/Pico1541/docs/dos1541-master/dos.bin").unwrap().read_to_end(&mut rom).unwrap();
    
    let mut emu = Emulator::new(&rom, 8, surface);
    let mut host = DummyHost;
    
    emu.reset(&mut host);
    
    let mut last_pcr = 0;
    for cycles in 0..10_000_000 {
        emu.tick(&mut host);
        
        let pcr = emu.memory.via2.pcr;
        if pcr != last_pcr {
            println!("Cycle {}: VIA2 PCR changed to {:02X}", emu.cpu.cycles, pcr);
            last_pcr = pcr;
        }
    }
}

