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
    pub iec_bus: IecBus,
}

impl HostBridge {
    fn new(event_tx: crossbeam_channel::Sender<DriveEvent>) -> Self {
        Self { active_track: ActiveTrack::new(), event_tx, iec_bus: IecBus(0xFF) }
    }
}

impl HostInterface for HostBridge {
    fn get_active_track(&mut self) -> &mut ActiveTrack { &mut self.active_track }
    fn send_event(&mut self, event: DriveEvent) { let _ = self.event_tx.send(event); }
    fn read_iec(&self) -> IecBus { self.iec_bus }
    fn write_iec(&mut self, pins: IecBus) { self.iec_bus = pins; }
}
use pico1541_manager::{D64Image, G64Image, MemoryStorage, DiskImage, AnyDiskImage};
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
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, pico1541_manager::Error>;
}

#[async_trait(?Send)]
impl<S: embedded_io_async::Read + embedded_io_async::Seek> DebuggerDiskImage for AnyDiskImage<S> {
    async fn read_track(&mut self, half_track: u8, dest: &mut [u8]) -> Result<u32, pico1541_manager::Error> {
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

    let input = Paragraph::new(app.input.as_str()).block(Block::default().borders(Borders::ALL).title("Command  F5:Run/Pause  F6:Step  F7:Reset  Esc:Quit"));
    f.render_widget(input, chunks[2]);
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
                    let storage = MemoryStorage::new(Box::leak(file_data.into_boxed_slice()));
                    let mut img = if path.to_lowercase().ends_with(".d64") { AnyDiskImage::D64(D64Image::new(storage)) }
                    else if path.to_lowercase().ends_with(".g64") { AnyDiskImage::G64(G64Image::new(storage)) }
                    else { return Err(anyhow!("Unsupported disk format")); };

                    // Load the initial track (track 18.0 = half-track 36)
                    let maybe_track = {
                        let mut temp_buf = vec![0u8; MAX_TRACK_BYTES];
                        let bit_len = DiskImage::read_track(&mut img, 36, &mut temp_buf).await.unwrap_or(0);
                        (bit_len, temp_buf)
                    };
                    {
                        let track = drive.cpu.bus.hardware.get_active_track();
                        track.gcr_data.copy_from_slice(&maybe_track.1);
                        track.bit_length = maybe_track.0;
                        track.disk_inserted = true;
                    }
                    disk_image = Some(Box::new(img));
                    resp_tx.send(DebugResponse::DriveEvent(format!("Mounted: {}", path)))?;
                }
                DebugCommand::Unmount => {
                    disk_image = None;
                    drive.cpu.bus.hardware.get_active_track().disk_inserted = false;
                    resp_tx.send(DebugResponse::DriveEvent("Unmounted".to_string()))?;
                }
                DebugCommand::IECCommand(s) => {
                    if s == "STATUS" {
                        let status = iec_read_status(&mut drive).await?;
                        resp_tx.send(DebugResponse::IECResponse(status))?;
                    } else if s.starts_with("LOAD:") {
                        let filename = &s[5..];
                        let data = iec_load_file(&mut drive, filename).await?;
                        if filename == "$" {
                            let dir = parse_directory(&data);
                            resp_tx.send(DebugResponse::IECResponse(dir))?;
                        } else {
                            resp_tx.send(DebugResponse::IECResponse(format!("Loaded {} bytes", data.len())))?;
                        }
                    }
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

async fn iec_read_status<H: HostInterface>(_drive: &mut Emulator<H>) -> Result<String> {
    Ok("00, OK, 00, 00".to_string())
}

async fn iec_load_file<H: HostInterface>(_drive: &mut Emulator<H>, _filename: &str) -> Result<Vec<u8>> {
    Ok(vec![])
}

fn parse_directory(data: &[u8]) -> String {
    if data.is_empty() { return "Empty directory".to_string(); }
    "Directory listing...".to_string()
}
