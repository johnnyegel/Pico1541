use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender, unbounded};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pico1541_emulator::{Emulator, interface::{ActiveTrack, DriveEvent, HostInterface, IecBus, MAX_TRACK_BYTES}};

struct HostBridge {
    pub active_track: ActiveTrack,
    event_tx: crossbeam_channel::Sender<DriveEvent>,
    /// What the host (debugger/C64) drives on the bus: 1=HIGH/released, 0=LOW/asserted
    pub host_drives: IecBus,
    /// Combined open-drain bus state written back by emulator each cycle
    pub bus_state: IecBus,
}

impl HostBridge {
    fn new(event_tx: crossbeam_channel::Sender<DriveEvent>) -> Self {
        let all_released = IecBus(IecBus::DATA | IecBus::CLK | IecBus::ATN);
        Self { active_track: ActiveTrack::new(), event_tx, host_drives: all_released, bus_state: all_released }
    }
}

impl HostInterface for HostBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.active_track }
    fn send_event(&mut self, event: DriveEvent) { let _ = self.event_tx.send(event); }
    fn read_iec(&self) -> IecBus { self.host_drives }
    fn write_iec(&mut self, pins: IecBus) { self.bus_state = pins; }
}
use pico1541_common::{D64Image, G64Image, MemoryStorage, DiskImage, AnyDiskImage};
use pico1541_common::disk_info::{inspect_d64, petscii_to_ascii, G64_MAGIC};
use racc6502::cpu::CycleState;
use racc6502::bus::MemoryMonitor;
use disasm6502;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, List, ListItem, ListState},
    Terminal,
};
use std::collections::HashSet;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use std::fs;
use tokio::runtime::Builder;
use tokio::task::LocalSet;
use async_trait::async_trait;

const ROM_BYTES: &[u8] = include_bytes!("../../../roms/1541-II.251968-03.bin");

#[derive(Debug)]
enum DebugCommand {
    Run,
    Pause,
    Step,
    Reset,
    SetBreakpoint(u16),
    ClearBreakpoint(u16),
    ReadMemory(u16, u16),
    Mount(String),
    Unmount,
    IECCommand(String),
    Exit,
}

#[derive(Debug, Clone)]
struct DisasmLine {
    addr: u16,
    text: String,
}

#[derive(Debug, Clone)]
struct CpuContext {
    pc: u16,
    a: u8,
    x: u8,
    y: u8,
    s: u8,
    p: u8,
    ht: u8,
    led: bool,
    motor: bool,
    disasm_window: Vec<DisasmLine>,
}

#[derive(Debug)]
enum DebugResponse {
    Context(CpuContext),
    BreakpointHit(CpuContext),
    DriveEvent(String),
    Memory(u16, Vec<u8>),
    IECResponse(String),
}

// Local dyn-compatible trait for the debugger.
// No Send bound because we use LocalSet.
#[async_trait(?Send)]
trait DebuggerDiskImage {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, pico1541_common::Error>;
}

#[async_trait(?Send)]
impl<S: embedded_io_async::Read + embedded_io_async::Seek> DebuggerDiskImage for AnyDiskImage<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, pico1541_common::Error> {
        DiskImage::read_track(self, half_track, dest).await
    }
}

struct App {
    log: Vec<String>,
    log_state: ListState,
    cpu_context: Option<CpuContext>,
    running: Arc<AtomicBool>,
    breakpoints: HashSet<u16>,
    input: String,
    memory_dump: Option<(u16, Vec<u8>)>,
}

impl App {
    fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            log: vec!["Debugger started.".to_string()],
            log_state: ListState::default(),
            cpu_context: None,
            running,
            breakpoints: HashSet::new(),
            input: String::new(),
            memory_dump: None,
        }
    }

    fn add_log(&mut self, s: String) {
        for line in s.lines() {
            self.log.push(line.to_string());
        }
        if self.log.len() > 1000 {
            self.log.remove(0);
        }
        self.log_state.select(Some(self.log.len().saturating_sub(1)));
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (cmd_tx, cmd_rx) = unbounded::<DebugCommand>();
    let (resp_tx, resp_rx) = unbounded::<DebugResponse>();

    let running = Arc::new(AtomicBool::new(false));
    let running_clone = Arc::clone(&running);

    thread::spawn(move || {
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        let local = LocalSet::new();
        local.block_on(&rt, async {
            if let Err(e) = emulation_loop(cmd_rx, resp_tx, running_clone).await {
                eprintln!("Emulation error: {:?}", e);
            }
        });
    });

    let mut app = App::new(running.clone());
    cmd_tx.send(DebugCommand::Pause)?;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        while let Ok(resp) = resp_rx.try_recv() {
            match resp {
                DebugResponse::Context(ctx) => { app.cpu_context = Some(ctx); }
                DebugResponse::BreakpointHit(ctx) => {
                    app.add_log(format!("Breakpoint hit at ${:04X}", ctx.pc));
                    app.cpu_context = Some(ctx);
                }
                DebugResponse::DriveEvent(s) => { app.add_log(s); }
                DebugResponse::Memory(addr, data) => { app.memory_dump = Some((addr, data)); }
                DebugResponse::IECResponse(s) => { app.add_log(format!("[IEC] {}", s)); }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Esc => {
                        cmd_tx.send(DebugCommand::Exit)?;
                        break;
                    }
                    KeyCode::F(5) => {
                        if app.running.load(Ordering::SeqCst) {
                            app.running.store(false, Ordering::SeqCst);
                            cmd_tx.send(DebugCommand::Pause)?;
                        } else {
                            app.running.store(true, Ordering::SeqCst);
                            cmd_tx.send(DebugCommand::Run)?;
                        }
                    }
                    KeyCode::F(6) => { cmd_tx.send(DebugCommand::Step)?; }
                    KeyCode::F(7) => {
                        cmd_tx.send(DebugCommand::Reset)?;
                        app.add_log("Drive Reset.".to_string());
                    }
                    KeyCode::Char(c) => { app.input.push(c); }
                    KeyCode::Backspace => { app.input.pop(); }
                    KeyCode::Enter => {
                        let cmd_str = app.input.clone();
                        if !cmd_str.is_empty() {
                            app.add_log(format!("> {}", cmd_str));
                            handle_command(cmd_str, &mut app, &cmd_tx)?;
                            app.input.clear();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_command(line: String, app: &mut App, cmd_tx: &Sender<DebugCommand>) -> Result<()> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() { return Ok(()); }

    match parts[0] {
        "run" | "r" => {
            app.running.store(true, Ordering::SeqCst);
            cmd_tx.send(DebugCommand::Run)?;
        }
        "pause" | "p" => {
            app.running.store(false, Ordering::SeqCst);
            cmd_tx.send(DebugCommand::Pause)?;
        }
        "step" | "s" => { cmd_tx.send(DebugCommand::Step)?; }
        "bp" => {
            if parts.len() < 2 {
                app.add_log("Usage: bp <addr>".to_string());
            } else {
                let addr = parse_addr(parts[1])?;
                if app.breakpoints.contains(&addr) {
                    app.breakpoints.remove(&addr);
                    cmd_tx.send(DebugCommand::ClearBreakpoint(addr))?;
                    app.add_log(format!("Cleared breakpoint at ${:04X}", addr));
                } else {
                    app.breakpoints.insert(addr);
                    cmd_tx.send(DebugCommand::SetBreakpoint(addr))?;
                    app.add_log(format!("Set breakpoint at ${:04X}", addr));
                }
            }
        }
        "mem" | "m" => {
            if parts.len() < 2 {
                app.add_log("Usage: mem <addr> [len]".to_string());
            } else {
                let addr = parse_addr(parts[1])?;
                let len = if parts.len() > 2 { parse_addr(parts[2])? } else { 16 };
                cmd_tx.send(DebugCommand::ReadMemory(addr, len))?;
            }
        }
        "mount" => {
            if parts.len() < 2 {
                app.add_log("Usage: mount <path>".to_string());
            } else {
                cmd_tx.send(DebugCommand::Mount(parts[1].to_string()))?;
            }
        }
        "unmount" => { cmd_tx.send(DebugCommand::Unmount)?; }
        "dir" | "ls" => { cmd_tx.send(DebugCommand::IECCommand("LOAD:$".to_string()))?; }
        "status" | "st" => { cmd_tx.send(DebugCommand::IECCommand("STATUS".to_string()))?; }
        "reset" => {
            cmd_tx.send(DebugCommand::Reset)?;
            app.add_log("Drive Reset.".to_string());
        }
        "help" | "h" => {
            app.add_log("Commands: r(un), p(ause), s(tep), bp <addr>, m(em) <addr> [len], mount <path>, unmount, dir, status, reset, exit".to_string());
        }
        "q" | "quit" | "exit" => { cmd_tx.send(DebugCommand::Exit)?; }
        _ => app.add_log(format!("Unknown command: {}", parts[0])),
    }
    Ok(())
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    let is_running = app.running.load(Ordering::SeqCst);
    let status_style = if is_running { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Yellow) };

    let status_text = if let Some(ctx) = &app.cpu_context {
        format!(
            " [{}]  PC:${:04X}  HT:{:<3}  LED:{}  Motor:{}",
            if is_running { "RUNNING" } else { "PAUSED " }, ctx.pc, ctx.ht,
            if ctx.led { "ON " } else { "OFF" }, if ctx.motor { "ON " } else { "OFF" }
        )
    } else {
        format!(" [{}]  Initializing...", if is_running { "RUNNING" } else { "PAUSED " })
    };

    let status_bar = Paragraph::new(status_text)
        .style(status_style)
        .block(Block::default().borders(Borders::ALL).title("Drive Status"));
    f.render_widget(status_bar, chunks[0]);

    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app.log.iter().map(|i| ListItem::new(i.as_str())).collect();
    let logs = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Events / History"))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(logs, middle_chunks[0], &mut app.log_state);

    let mut context_lines = Vec::new();
    if let Some(ctx) = &app.cpu_context {
        let p = ctx.p;
        let flags = format!("{}{}-{}{}{}{}{}",
            if p & 0x80 != 0 { 'N' } else { 'n' }, if p & 0x40 != 0 { 'V' } else { 'v' },
            if p & 0x10 != 0 { 'B' } else { 'b' }, if p & 0x08 != 0 { 'D' } else { 'd' },
            if p & 0x04 != 0 { 'I' } else { 'i' }, if p & 0x02 != 0 { 'Z' } else { 'z' },
            if p & 0x01 != 0 { 'C' } else { 'c' },
        );

        context_lines.push(Line::from(vec![Span::styled("Registers: ", Style::default().add_modifier(Modifier::BOLD))]));
        context_lines.push(Line::from(format!("  A:${:02X} X:${:02X} Y:${:02X} S:${:02X}", ctx.a, ctx.x, ctx.y, ctx.s)));
        context_lines.push(Line::from(format!("  P:${:02X} [{}]", ctx.p, flags)));
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![Span::styled("Disassembly: ", Style::default().add_modifier(Modifier::BOLD))]));

        for line in &ctx.disasm_window {
            let style = if line.addr == ctx.pc { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
            let prefix = if line.addr == ctx.pc { ">> " } else { "   " };
            context_lines.push(Line::from(vec![Span::styled(format!("{}{}", prefix, line.text), style)]));
        }
    }

    if let Some((addr, data)) = &app.memory_dump {
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![Span::styled(format!("Memory Dump at ${:04X}:", addr), Style::default().add_modifier(Modifier::BOLD))]));
        for (i, chunk) in data.chunks(8).enumerate() {
            let mut line_str = format!("  ${:04X}: ", addr.wrapping_add(i as u16 * 8));
            for b in chunk { line_str.push_str(&format!("{:02X} ", b)); }
            context_lines.push(Line::from(line_str));
        }
    }

    let context_box = Paragraph::new(context_lines).block(Block::default().borders(Borders::ALL).title("CPU Context"));
    f.render_widget(context_box, middle_chunks[1]);

    let input_text = format!("> {}", app.input);
    let input = Paragraph::new(input_text.as_str()).block(Block::default().borders(Borders::ALL).title("Command  F5:Run/Pause  F6:Step  F7:Reset  Esc:Quit"));
    f.render_widget(input, chunks[2]);
    f.set_cursor_position((
        chunks[2].x + 1 + 2 + app.input.len() as u16, // border + "> " + typed text
        chunks[2].y + 1,                                // border
    ));
}

fn parse_addr(s: &str) -> Result<u16> {
    if let Some(stripped) = s.strip_prefix("0x") { u16::from_str_radix(stripped, 16).map_err(Into::into) }
    else if let Some(stripped) = s.strip_prefix('$') { u16::from_str_radix(stripped, 16).map_err(Into::into) }
    else { s.parse().map_err(Into::into) }
}

fn get_cpu_context<H: HostInterface>(drive: &Emulator<H>) -> CpuContext {
    let pc = drive.pc();
    let start_addr = pc.saturating_sub(16);
    let mut buffer = [0u8; 48];
    for i in 0..48 { buffer[i] = drive.bus().peek(start_addr.wrapping_add(i as u16)); }
    let instructions = disasm6502::from_addr_array(&buffer, start_addr).unwrap_or_default();
    let mut disasm_window = Vec::new();
    for inst in instructions {
        disasm_window.push(DisasmLine { addr: inst.address, text: format!("${:04X}: {}", inst.address, inst.as_str()) });
    }
    CpuContext {
        pc, a: drive.cpu.a, x: drive.cpu.x, y: drive.cpu.y, s: drive.cpu.s, p: drive.cpu.p,
        ht: drive.half_track(), led: drive.bus().led_on(), motor: drive.bus().motor_on(), disasm_window
    }
}

async fn emulation_loop(
    cmd_rx: Receiver<DebugCommand>,
    resp_tx: Sender<DebugResponse>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(ROM_BYTES);

    let (event_tx, event_rx) = unbounded::<DriveEvent>();
    let bridge = HostBridge::new(event_tx);
    let mut drive = Emulator::new(rom_array, bridge);
    drive.reset();

    let mut disk_image: Option<Box<dyn DebuggerDiskImage>> = None;
    let mut breakpoints: HashSet<u16> = HashSet::new();
    let mut last_tick = Instant::now();
    let mut cycle_accumulator = 0.0;
    let cycles_per_sec = 1_000_000.0;

    loop {
        while let Ok(cmd) = if running.load(Ordering::SeqCst) { cmd_rx.try_recv() } else { cmd_rx.recv().map_err(|_| crossbeam_channel::TryRecvError::Disconnected) } {
            match cmd {
                DebugCommand::Run => { running.store(true, Ordering::SeqCst); last_tick = Instant::now(); }
                DebugCommand::Pause => { running.store(false, Ordering::SeqCst); resp_tx.send(DebugResponse::Context(get_cpu_context(&drive)))?; }
                DebugCommand::Step => { drive.step(); resp_tx.send(DebugResponse::Context(get_cpu_context(&drive)))?; }
                DebugCommand::Reset => { drive.reset(); resp_tx.send(DebugResponse::Context(get_cpu_context(&drive)))?; }
                DebugCommand::SetBreakpoint(addr) => { breakpoints.insert(addr); }
                DebugCommand::ClearBreakpoint(addr) => { breakpoints.remove(&addr); }
                DebugCommand::ReadMemory(addr, len) => {
                    let mut data = Vec::with_capacity(len as usize);
                    for i in 0..len { data.push(drive.bus().peek(addr.wrapping_add(i))); }
                    resp_tx.send(DebugResponse::Memory(addr, data))?;
                }
                DebugCommand::Mount(path) => {
                    let file_data = fs::read(&path)?;
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
                            if valid { "OK - known valid size" } else { "WARN - unexpected file size" }
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
                        AnyDiskImage::D64(d64)
                    } else if path_lower.ends_with(".g64") {
                        let valid = file_data.len() >= 8 && file_data[..8] == *G64_MAGIC;
                        resp_tx.send(DebugResponse::DriveEvent(format!(
                            "Type: G64  Size: {} bytes  {}",
                            file_data.len(),
                            if valid { "OK - valid header" } else { "WARN - header mismatch" }
                        )))?;
                        let storage = MemoryStorage::new(Box::leak(file_data.into_boxed_slice()));
                        AnyDiskImage::G64(G64Image::new(storage))
                    } else {
                        return Err(anyhow!("Unsupported disk format"));
                    };

                    // Load track 18.0 (half-track 36) as the initial active track
                    let (bit_len, temp_buf) = {
                        let mut buf = vec![0u8; MAX_TRACK_BYTES];
                        let bits = DiskImage::read_track(&mut img, 36, &mut buf).await.unwrap_or(0);
                        (bits, buf)
                    };
                    {
                        let track = drive.cpu.bus.hardware.get_active_track();
                        track.gcr_data.copy_from_slice(&temp_buf);
                        track.bit_length = bit_len;
                        track.disk_inserted = true;
                    }
                    resp_tx.send(DebugResponse::DriveEvent(format!(
                        "Track 18 loaded: {} bits ({} bytes of GCR)",
                        bit_len, bit_len / 8
                    )))?;
                    resp_tx.send(DebugResponse::DriveEvent(format!("Mounted: {}", path)))?;
                    disk_image = Some(Box::new(img));
                }
                DebugCommand::Unmount => {
                    disk_image = None;
                    drive.cpu.bus.hardware.get_active_track().disk_inserted = false;
                    resp_tx.send(DebugResponse::DriveEvent("Unmounted".to_string()))?;
                }
                DebugCommand::IECCommand(s) => {
                    // Pause emulation while doing IEC – the IEC functions run cycles themselves
                    let was_running = running.load(Ordering::SeqCst);
                    running.store(false, Ordering::SeqCst);

                    let iec_result = if s == "STATUS" {
                        iec_read_status(&mut drive).await
                            .map(|status| DebugResponse::IECResponse(status))
                    } else if s.starts_with("LOAD:") {
                        let filename = s[5..].to_string();
                        iec_load_file(&mut drive, &filename).await.map(|data| {
                            if filename == "$" {
                                DebugResponse::IECResponse(parse_directory(&data))
                            } else {
                                DebugResponse::IECResponse(format!("Loaded {} bytes", data.len()))
                            }
                        })
                    } else {
                        Err(anyhow!("Unknown IEC command: {}", s))
                    };

                    match iec_result {
                        Ok(resp) => { resp_tx.send(resp)?; }
                        Err(e) => { resp_tx.send(DebugResponse::IECResponse(format!("IEC error: {}", e)))?; }
                    }

                    if was_running { running.store(true, Ordering::SeqCst); last_tick = Instant::now(); }
                }
                DebugCommand::Exit => return Ok(()),
            }
        }

        if running.load(Ordering::SeqCst) {
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick);
            last_tick = now;
            cycle_accumulator += elapsed.as_secs_f64() * cycles_per_sec;
            let mut cycles_to_run = cycle_accumulator as u32;
            cycle_accumulator -= cycles_to_run as f64;

            while cycles_to_run > 0 {
                drive.cpu.tick();
                cycles_to_run -= 1;
                if drive.cpu.state == CycleState::FetchOpcode && drive.cpu.step_index == 0 {
                    if breakpoints.contains(&drive.pc()) {
                        running.store(false, Ordering::SeqCst);
                        resp_tx.send(DebugResponse::BreakpointHit(get_cpu_context(&drive)))?;
                        break;
                    }
                }
            }

            // Process drive events from the HostBridge
            while let Ok(event) = event_rx.try_recv() {
                match event {
                    DriveEvent::HeadStepped { new_half_track } => {
                        let maybe_track = if let Some(img) = &mut disk_image {
                            let mut temp_buf = vec![0u8; MAX_TRACK_BYTES];
                            let bit_len = img.read_track(new_half_track, &mut temp_buf).await.unwrap_or(0);
                            Some((bit_len, temp_buf))
                        } else {
                            None
                        };
                        if let Some((bit_len, temp_buf)) = maybe_track {
                            let track = drive.cpu.bus.hardware.get_active_track();
                            track.gcr_data.copy_from_slice(&temp_buf);
                            track.bit_length = bit_len;
                        }
                        resp_tx.send(DebugResponse::DriveEvent(format!("Head: HT={}", new_half_track)))?;
                    }
                    DriveEvent::LedChanged(on) => {
                        resp_tx.send(DebugResponse::DriveEvent(format!("LED: {}", if on { "ON" } else { "OFF" })))?;
                    }
                    DriveEvent::MotorChanged(on) => {
                        resp_tx.send(DebugResponse::DriveEvent(format!("Motor: {}", if on { "ON" } else { "OFF" })))?;
                    }
                }
            }

            if cycles_to_run == 0 { thread::sleep(Duration::from_micros(100)); }
        } else { thread::sleep(Duration::from_millis(10)); }
    }
}

// ─── IEC Bus helpers ─────────────────────────────────────────────────────────

const IEC_TIMEOUT: u32 = 1_000_000; // 1 second worth of cycles at 1 MHz
const IEC_BIT_DELAY: u32 = 20;      // ~20µs per half-bit
const IEC_EOI_THRESHOLD: u32 = 300; // ~300µs marks EOI

fn iec_tick(drive: &mut Emulator<HostBridge>) {
    drive.cpu.tick();
}

fn iec_run(drive: &mut Emulator<HostBridge>, n: u32) {
    for _ in 0..n { drive.cpu.tick(); }
}

fn iec_bus(drive: &Emulator<HostBridge>) -> IecBus {
    drive.cpu.bus.hardware.bus_state
}

fn iec_set_data(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_data(high);
}
fn iec_set_clk(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_clk(high);
}
fn iec_set_atn(drive: &mut Emulator<HostBridge>, high: bool) {
    drive.cpu.bus.hardware.host_drives.set_atn(high);
}

/// Wait running emulator until predicate returns true on the combined bus state.
fn iec_wait(drive: &mut Emulator<HostBridge>, pred: impl Fn(IecBus) -> bool) -> Result<()> {
    for _ in 0..IEC_TIMEOUT {
        iec_tick(drive);
        if pred(iec_bus(drive)) { return Ok(()); }
    }
    Err(anyhow!("IEC timeout"))
}

/// Assert ATN and wait for drive to acknowledge by pulling DATA LOW.
fn iec_atn_start(drive: &mut Emulator<HostBridge>) -> Result<()> {
    iec_set_atn(drive, false); // assert ATN
    iec_set_clk(drive, false); // host holds CLK during ATN
    iec_run(drive, 100);
    // Drive should assert DATA within ~1ms
    iec_wait(drive, |b| !b.data())
}

/// Release ATN. Drive continues as current TALKER/LISTENER role.
fn iec_atn_release(drive: &mut Emulator<HostBridge>) {
    iec_set_atn(drive, true);
    iec_run(drive, 100);
}

/// Send one byte with ATN already asserted (host is sender).
fn iec_send_byte_under_atn(drive: &mut Emulator<HostBridge>, byte: u8) -> Result<()> {
    // Release CLK to signal "ready to send byte"
    iec_set_clk(drive, true);
    // Wait for drive DATA HIGH = drive ready to receive byte
    iec_wait(drive, |b| b.data())?;
    // Assert CLK briefly (setup)
    iec_set_clk(drive, false);
    iec_run(drive, IEC_BIT_DELAY * 2);

    // Send 8 bits LSB-first
    for bit_idx in 0..8u8 {
        let bit = (byte >> bit_idx) & 1 != 0;
        iec_set_data(drive, bit);           // set DATA to bit value
        iec_set_clk(drive, true);           // CLK HIGH = bit valid
        iec_run(drive, IEC_BIT_DELAY);
        iec_set_clk(drive, false);          // CLK LOW = bit done
        iec_run(drive, IEC_BIT_DELAY);
    }
    // Release DATA
    iec_set_data(drive, true);
    // Wait for drive DATA LOW = byte acknowledged
    iec_wait(drive, |b| !b.data())?;
    // Reassert CLK for next byte
    iec_set_clk(drive, false);
    iec_run(drive, 50);
    Ok(())
}

/// Perform TALK + secondary address handshake, then receive bytes until EOI.
/// Call iec_atn_start() before this, then release ATN after last command byte.
fn iec_receive_data(drive: &mut Emulator<HostBridge>) -> Result<Vec<u8>> {
    // Talker turnaround: drive takes CLK, host releases CLK
    iec_set_clk(drive, true);  // host releases CLK
    iec_set_data(drive, true); // host releases DATA initially
    iec_run(drive, 200);       // wait for drive to set up

    let mut result = Vec::new();

    loop {
        // --- per-byte receive ---
        // Assert DATA = "not ready yet"
        iec_set_data(drive, false);
        iec_run(drive, 50);
        // Release DATA = "ready to receive"
        iec_set_data(drive, true);

        // Wait for drive to release CLK (drive is ready to send byte / signal EOI)
        iec_wait(drive, |b| b.clk())?;

        // Check for EOI: if CLK stays HIGH > threshold, it's the last byte
        let mut eoi = false;
        let mut high_count = 0u32;
        loop {
            iec_tick(drive);
            if !iec_bus(drive).clk() {
                break; // CLK went LOW, sending started
            }
            high_count += 1;
            if high_count > IEC_EOI_THRESHOLD {
                // EOI: acknowledge by asserting DATA ~60µs, then release
                iec_set_data(drive, false);
                iec_run(drive, 60);
                iec_set_data(drive, true);
                eoi = true;
                // Now wait for drive to actually start sending
                iec_wait(drive, |b| !b.clk())?;
                break;
            }
        }

        // Read 8 bits LSB-first: each bit valid when CLK HIGH
        let mut byte_val: u8 = 0;
        for bit_idx in 0..8u8 {
            // Wait for CLK LOW→HIGH (bit valid)
            iec_wait(drive, |b| b.clk())?;
            if iec_bus(drive).data() { byte_val |= 1 << bit_idx; }
            // Wait for CLK HIGH→LOW (bit done)
            iec_wait(drive, |b| !b.clk())?;
        }
        result.push(byte_val);

        // Acknowledge byte
        iec_set_data(drive, false);
        iec_run(drive, 60);
        iec_set_data(drive, true);

        if eoi { break; }
    }

    Ok(result)
}

/// Read the 1541 status channel (channel 15).
async fn iec_read_status(drive: &mut Emulator<HostBridge>) -> Result<String> {
    // TALK 0x48, OPEN_CHANNEL 0x6F (sa=15)
    iec_atn_start(drive)?;
    iec_send_byte_under_atn(drive, 0x48)?; // TALK device 8
    iec_send_byte_under_atn(drive, 0x6F)?; // DATA channel 15
    iec_atn_release(drive);

    let data = iec_receive_data(drive)?;

    // Restore bus to idle
    iec_set_data(drive, true);
    iec_set_clk(drive, true);
    iec_set_atn(drive, true);

    Ok(String::from_utf8_lossy(&data).trim_end_matches('\r').to_string())
}

/// Load a file by name from the 1541 (LISTEN→OPEN→UNLISTEN→TALK→DATA→receive).
async fn iec_load_file(drive: &mut Emulator<HostBridge>, filename: &str) -> Result<Vec<u8>> {
    // Step 1: LISTEN + OPEN channel 0 (secondary 0xF0), send filename
    iec_atn_start(drive)?;
    iec_send_byte_under_atn(drive, 0x28)?; // LISTEN device 8
    iec_send_byte_under_atn(drive, 0xF0)?; // OPEN channel 0
    iec_atn_release(drive);

    // Send filename bytes (drive is now LISTENER)
    // For sending as LISTENER: drive holds CLK, we send via DATA
    // Actually after ATN released with LISTEN: host sends filename bytes
    // using the standard byte send protocol (but host holds CLK now)
    iec_set_clk(drive, false); // host takes CLK
    for &b in filename.as_bytes() {
        iec_send_byte_under_atn(drive, b)?;
    }

    // Step 2: UNLISTEN
    iec_atn_start(drive)?;
    iec_send_byte_under_atn(drive, 0x3F)?; // UNLISTEN
    iec_atn_release(drive);

    // Step 3: TALK + DATA channel 0
    iec_atn_start(drive)?;
    iec_send_byte_under_atn(drive, 0x48)?; // TALK device 8
    iec_send_byte_under_atn(drive, 0x60)?; // DATA channel 0
    iec_atn_release(drive);

    let data = iec_receive_data(drive)?;

    // Restore bus to idle
    iec_set_data(drive, true);
    iec_set_clk(drive, true);
    iec_set_atn(drive, true);

    Ok(data)
}

fn parse_directory(data: &[u8]) -> String {
    if data.len() < 2 {
        return "(empty directory data)".to_string();
    }
    let mut lines = Vec::new();
    let mut pos = 2; // skip load address
    while pos + 4 <= data.len() {
        let _next_link = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if _next_link == 0 { break; }
        let line_num = u16::from_le_bytes([data[pos + 2], data[pos + 3]]);
        pos += 4;
        // Collect PETSCII bytes until 0x00
        let mut text = String::new();
        while pos < data.len() && data[pos] != 0x00 {
            let c = data[pos];
            // PETSCII to ASCII: uppercase letters are 0x41-0x5A
            let ascii = match c {
                0x20..=0x5F => c,           // printable ASCII range
                0x60..=0x7E => c - 0x20,    // PETSCII lowercase → uppercase
                0xC1..=0xDA => c - 0x80,    // PETSCII uppercase shifted
                _ => b'.',
            };
            text.push(ascii as char);
            pos += 1;
        }
        pos += 1; // skip null terminator
        lines.push(format!("{:5} {}", line_num, text));
    }
    if lines.is_empty() { "(no entries)".to_string() } else { lines.join("\n") }
}
