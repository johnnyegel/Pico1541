use anyhow::{anyhow, Result};
use async_trait::async_trait;
use crossbeam_channel::{Receiver, Sender};
use disasm6502;
use pico1541_common::{AnyDiskImage, D64Image, DiskImage, G64Image, MemoryStorage};
use pico1541_common::storage::DummySource;
use pico1541_common::disk_info::{inspect_d64, petscii_to_ascii, G64_MAGIC};
use pico1541_emulator::{interface::{DriveEvent, HostInterface, MAX_TRACK_BYTES}, Emulator};
use racc6502::bus::MemoryMonitor;
use racc6502::cpu::CycleState;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};
use std::fs;

use crate::bridge::{
    CpuContext, DebugCommand, DebugResponse, DisasmLine, HostBridge,
    LockStepCommand, LockStepResponse, BusSnapshot
};
use crate::iec::{iec_load_file, iec_read_status, parse_directory, LockStepBus};

pub const ROM_BYTES: &[u8] = include_bytes!("../../../roms/1541-II.251968-03.bin");

// ─── Disk image abstraction ───────────────────────────────────────────────────

/// Dyn-compatible disk image trait for the debugger (no Send required — LocalSet).
#[async_trait(?Send)]
trait DebuggerDiskImage {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<(u32, bool), pico1541_common::Error>;
}

#[async_trait(?Send)]
impl<S: embedded_io_async::Read + embedded_io_async::Seek> DebuggerDiskImage for AnyDiskImage<S, DummySource> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<(u32, bool), pico1541_common::Error> {
        DiskImage::read_track(self, half_track, dest).await
    }
}

// ─── CPU context snapshot ────────────────────────────────────────────────────

pub fn get_cpu_context(drive: &Emulator<HostBridge>, bridge: &HostBridge) -> CpuContext {
    let _ = bridge; // reserved for future hardware-side fields
    let pc = drive.pc();
    let start_addr = pc.saturating_sub(16);
    let mut buffer = [0u8; 48];
    for i in 0..48 {
        buffer[i] = drive.bus().peek(start_addr.wrapping_add(i as u16));
    }
    let instructions = disasm6502::from_addr_array(&buffer, start_addr).unwrap_or_default();
    let mut disasm_window = Vec::new();
    for inst in instructions {
        disasm_window.push(DisasmLine {
            addr: inst.address,
            text: format!("${:04X}: {}", inst.address, inst.as_str()),
        });
    }
    CpuContext {
        pc,
        a: drive.cpu.a, x: drive.cpu.x, y: drive.cpu.y, s: drive.cpu.s, p: drive.cpu.p,
        ht: drive.half_track(),
        led: drive.bus().led_on(),
        motor: drive.bus().motor_on(),
        disasm_window,
    }
}

// ─── Emulation loop ───────────────────────────────────────────────────────────

pub async fn emulation_loop(
    cmd_rx: Receiver<DebugCommand>,
    resp_tx: Sender<DebugResponse>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(ROM_BYTES);

    let (event_tx, event_rx) = crossbeam_channel::unbounded::<DriveEvent>();
    let mut bridge = HostBridge::new(event_tx);
    let mut drive = Emulator::<HostBridge>::new(rom_array);
    drive.reset();

    let mut disk_image: Option<Box<dyn DebuggerDiskImage>> = None;
    let mut breakpoints: HashSet<u16> = HashSet::new();
    let mut last_tick = Instant::now();
    let mut cycle_accumulator = 0.0f64;
    let cycles_per_sec = 1_000_000.0f64;

    loop {
        // ── Command dispatch ──────────────────────────────────────────────────
        while let Ok(cmd) = if running.load(Ordering::SeqCst) {
            cmd_rx.try_recv()
        } else {
            cmd_rx.recv().map_err(|_| crossbeam_channel::TryRecvError::Disconnected)
        } {
            match cmd {
                DebugCommand::Run => {
                    running.store(true, Ordering::SeqCst);
                    last_tick = Instant::now();
                }
                DebugCommand::Pause => {
                    running.store(false, Ordering::SeqCst);
                    resp_tx.send(DebugResponse::Context(get_cpu_context(&drive, &bridge)))?;
                }
                DebugCommand::Step => {
                    drive.step(&mut bridge);
                    resp_tx.send(DebugResponse::Context(get_cpu_context(&drive, &bridge)))?;
                }
                DebugCommand::Reset => {
                    drive.reset();
                    resp_tx.send(DebugResponse::Context(get_cpu_context(&drive, &bridge)))?;
                }
                DebugCommand::SetBreakpoint(addr) => { breakpoints.insert(addr); }
                DebugCommand::ClearBreakpoint(addr) => { breakpoints.remove(&addr); }
                DebugCommand::ReadMemory(addr, len) => {
                    let mut data = Vec::with_capacity(len as usize);
                    for i in 0..len { data.push(drive.bus().peek(addr.wrapping_add(i))); }
                    resp_tx.send(DebugResponse::Memory(addr, data))?;
                }

                DebugCommand::Mount(path) => {
                    handle_mount(&path, &mut drive, &mut bridge, &mut disk_image, &resp_tx).await?;
                }
                DebugCommand::Unmount => {
                    disk_image = None;
                    bridge.get_active_track().disk_inserted = false;
                    resp_tx.send(DebugResponse::DriveEvent("Unmounted".to_string()))?;
                }

                DebugCommand::IECCommand(s) => {
                    let (ls_cmd_tx, ls_cmd_rx) = crossbeam_channel::unbounded();
                    let (ls_resp_tx, ls_resp_rx) = crossbeam_channel::unbounded();
                    bridge.set_lockstep(ls_cmd_rx, ls_resp_tx);

                    let resp_tx_clone = resp_tx.clone();
                    tokio::task::spawn_local(async move {
                        let mut bus = LockStepBus {
                            cmd_tx: ls_cmd_tx,
                            resp_rx: ls_resp_rx,
                            host_drives: BusSnapshot::default(),
                            last_pc: 0,
                        };

                        let result = if s == "STATUS" {
                            iec_read_status(&mut bus)
                                .map(|status| DebugResponse::IECResponse(status))
                        } else if s.starts_with("LOAD:") {
                            let filename = s[5..].to_string();
                            iec_load_file(&mut bus, &filename).map(|data| {
                                if filename == "$" {
                                    DebugResponse::IECResponse(parse_directory(&data))
                                } else {
                                    DebugResponse::IECResponse(format!("Loaded {} bytes", data.len()))
                                }
                            })
                        } else {
                            Err(anyhow!("Unknown IEC command: {}", s))
                        };

                        match result {
                            Ok(resp) => { let _ = resp_tx_clone.send(resp); }
                            Err(e) => {
                                let _ = resp_tx_clone.send(DebugResponse::IECResponse(format!("IEC error: {}", e)));
                            }
                        }
                    });
                }
                DebugCommand::Exit => return Ok(()),
            }
        }

        // ── LockStep synchronization ──────────────────────────────────────────
        let lockstep_rx = bridge.lockstep_rx.clone();
        if let Some(rx) = lockstep_rx {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    LockStepCommand::Step { host_drives, cycles } => {
                        bridge.host_drives = host_drives;
                        for _ in 0..cycles { drive.tick(&mut bridge); }
                        let _ = bridge.lockstep_tx.as_ref().unwrap()
                            .send(LockStepResponse::Done(bridge.bus_state, drive.pc()));
                    }
                    LockStepCommand::Wait { host_drives, target, mask, timeout_cycles } => {
                        bridge.host_drives = host_drives;
                        let mut timed_out = true;
                        for _ in 0..timeout_cycles {
                            drive.tick(&mut bridge);
                            let bus = bridge.bus_state;
                            if (bus.data == target.data || !mask.data) &&
                               (bus.clk  == target.clk  || !mask.clk)  &&
                               (bus.atn  == target.atn  || !mask.atn) {
                                let _ = bridge.lockstep_tx.as_ref().unwrap()
                                    .send(LockStepResponse::Done(bus, drive.pc()));
                                timed_out = false;
                                break;
                            }
                        }
                        if timed_out {
                            let _ = bridge.lockstep_tx.as_ref().unwrap()
                                .send(LockStepResponse::Timeout(bridge.bus_state, drive.pc()));
                        }
                    }
                }
            }
        }

        // ── Emulation run ─────────────────────────────────────────────────────
        if running.load(Ordering::SeqCst) {
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick);
            last_tick = now;
            cycle_accumulator += elapsed.as_secs_f64() * cycles_per_sec;
            let mut cycles_to_run = cycle_accumulator as u32;
            cycle_accumulator -= cycles_to_run as f64;

            while cycles_to_run > 0 {
                drive.tick(&mut bridge);
                cycles_to_run -= 1;
                if drive.cpu.state == CycleState::FetchOpcode && drive.cpu.step_index == 0 {
                    if breakpoints.contains(&drive.pc()) {
                        running.store(false, Ordering::SeqCst);
                        resp_tx.send(DebugResponse::BreakpointHit(get_cpu_context(&drive, &bridge)))?;
                        break;
                    }
                }
            }

            // Process drive events forwarded from HostBridge
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    DriveEvent::HeadStepped { new_half_track } => {
                        let maybe_track = if let Some(img) = &mut disk_image {
                            let mut temp_buf = vec![0u8; MAX_TRACK_BYTES];
                            let (bit_len, accurate) = img.read_track(new_half_track, &mut temp_buf).await.unwrap_or((0, true));
                            Some((bit_len, accurate, temp_buf))
                        } else {
                            None
                        };
                        if let Some((bit_len, accurate, temp_buf)) = maybe_track {
                            let track = bridge.get_active_track();
                            track.gcr_data.copy_from_slice(&temp_buf);
                            track.bit_length = bit_len;
                            track.is_accurate = accurate;
                        }
                        resp_tx.send(DebugResponse::DriveEvent(
                            format!("Head: HT={}", new_half_track)
                        ))?;
                    }
                    DriveEvent::LedChanged(on) => {
                        resp_tx.send(DebugResponse::DriveEvent(
                            format!("LED: {}", if on { "ON" } else { "OFF" })
                        ))?;
                    }
                    DriveEvent::MotorChanged(on) => {
                        resp_tx.send(DebugResponse::DriveEvent(
                            format!("Motor: {}", if on { "ON" } else { "OFF" })
                        ))?;
                    }
                }
            }

            if cycles_to_run == 0 { thread::sleep(Duration::from_micros(100)); }
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

// ─── Mount handler ────────────────────────────────────────────────────────────

async fn handle_mount(
    path: &str,
    drive: &mut Emulator<HostBridge>,
    bridge: &mut HostBridge,
    disk_image: &mut Option<Box<dyn DebuggerDiskImage>>,
    resp_tx: &Sender<DebugResponse>,
) -> Result<()> {
    let file_data = fs::read(path)?;
    let path_lower = path.to_lowercase();

    let mut img = if path_lower.ends_with(".d64") {
        let info = inspect_d64(&file_data);
        let (valid, disk_id) = info.as_ref()
            .map(|i| (i.valid_size, i.disk_id))
            .unwrap_or((false, [0x31, 0x31]));
        let disk_name = info.as_ref().map(|i| {
            let mut buf = [0u8; 16];
            let len = petscii_to_ascii(&i.disk_name_petscii, &mut buf);
            String::from_utf8_lossy(&buf[..len]).into_owned()
        }).unwrap_or_else(|| "(file too small to read BAM)".to_string());

        resp_tx.send(DebugResponse::DriveEvent(format!(
            "Type: D64  Size: {} bytes ({} KB)  {}",
            file_data.len(), file_data.len() / 1024,
            if valid { "OK - known valid size" } else { "WARN - unexpected file size" },
        )))?;
        resp_tx.send(DebugResponse::DriveEvent(format!(
            "Disk name: \"{}\"  ID: {:02X} {:02X} ('{}{}')",
            disk_name, disk_id[0], disk_id[1],
            disk_id[0].clamp(0x20, 0x7E) as char,
            disk_id[1].clamp(0x20, 0x7E) as char,
        )))?;

        let storage = MemoryStorage::new(Box::leak(file_data.into_boxed_slice()));
        let mut d64 = D64Image::new(storage);
        d64.set_id(disk_id);
        AnyDiskImage::<_, DummySource>::D64(d64)

    } else if path_lower.ends_with(".g64") {
        let valid = file_data.len() >= 8 && file_data[..8] == *G64_MAGIC;
        resp_tx.send(DebugResponse::DriveEvent(format!(
            "Type: G64  Size: {} bytes  {}",
            file_data.len(),
            if valid { "OK - valid header" } else { "WARN - header mismatch" },
        )))?;
        let storage = MemoryStorage::new(Box::leak(file_data.into_boxed_slice()));
        AnyDiskImage::<_, DummySource>::G64(G64Image::new(storage))

    } else {
        return Err(anyhow!("Unsupported disk format"));
    };

    // Load track 18 (half-track 36) as the initial active track
    let (bit_len, accurate, temp_buf) = {
        let mut buf = vec![0u8; MAX_TRACK_BYTES];
        let (bits, acc) = DiskImage::read_track(&mut img, 36, &mut buf).await.unwrap_or((0, true));
        (bits, acc, buf)
    };
    {
        let track = bridge.get_active_track();
        track.gcr_data.copy_from_slice(&temp_buf);
        track.bit_length = bit_len;
        track.is_accurate = accurate;
        track.disk_inserted = true;
    }
    let _ = drive; // drive is not needed here after the refactor
    resp_tx.send(DebugResponse::DriveEvent(format!(
        "Track 18 loaded: {} bits ({} bytes of GCR)", bit_len, bit_len / 8
    )))?;
    resp_tx.send(DebugResponse::DriveEvent(format!("Mounted: {}", path)))?;
    *disk_image = Some(Box::new(img));
    Ok(())
}
