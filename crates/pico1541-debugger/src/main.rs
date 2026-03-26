use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, unbounded};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pico1541_emu::activetrack::ActiveTrack;
use pico1541_emu::{Command as EmuCommand, DriveHardware, Emulator, HardwareBridge};
use racc6502::cpu::CycleState;
use racc6502::disasm;
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
    Exit,
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
    disasm_prev: String,
    disasm_next: String,
}

#[derive(Debug)]
enum DebugResponse {
    Context(CpuContext),
    BreakpointHit(CpuContext),
    DriveEvent(String),
    Memory(u16, Vec<u8>),
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
            log: vec!["Debugger started. F5:Run/Pause  F10:Step  F12:Reset".to_string()],
            log_state: ListState::default(),
            cpu_context: None,
            running,
            breakpoints: HashSet::new(),
            input: String::new(),
            memory_dump: None,
        }
    }

    fn add_log(&mut self, s: String) {
        self.log.push(s);
        if self.log.len() > 1000 {
            self.log.remove(0);
        }
        self.log_state.select(Some(self.log.len().saturating_sub(1)));
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup communication
    let (cmd_tx, cmd_rx) = unbounded::<DebugCommand>();
    let (resp_tx, resp_rx) = unbounded::<DebugResponse>();

    let running = Arc::new(AtomicBool::new(false));
    let running_clone = Arc::clone(&running);

    // Spawn emulation thread
    thread::spawn(move || {
        if let Err(e) = emulation_loop(cmd_rx, resp_tx, running_clone) {
            eprintln!("Emulation error: {:?}", e);
        }
    });

    let mut app = App::new(running.clone());

    // Initial status request
    cmd_tx.send(DebugCommand::Pause)?;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        // Handle responses from emulation thread
        while let Ok(resp) = resp_rx.try_recv() {
            match resp {
                DebugResponse::Context(ctx) => {
                    app.cpu_context = Some(ctx);
                }
                DebugResponse::BreakpointHit(ctx) => {
                    app.add_log(format!("Breakpoint hit at ${:04X}", ctx.pc));
                    app.cpu_context = Some(ctx);
                }
                DebugResponse::DriveEvent(s) => {
                    app.add_log(s);
                }
                DebugResponse::Memory(addr, data) => {
                    app.memory_dump = Some((addr, data));
                }
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
                    KeyCode::F(10) => {
                        cmd_tx.send(DebugCommand::Step)?;
                    }
                    KeyCode::F(12) => {
                        cmd_tx.send(DebugCommand::Reset)?;
                        app.add_log("Drive Reset.".to_string());
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
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

    // Restore terminal
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
        "step" | "s" => {
            cmd_tx.send(DebugCommand::Step)?;
        }
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
        "reset" => {
            cmd_tx.send(DebugCommand::Reset)?;
            app.add_log("Drive Reset.".to_string());
        }
        "help" | "h" => {
            app.add_log("Commands: r(un), p(ause), s(tep), bp <addr>, m(em) <addr> [len], reset, exit".to_string());
        }
        "q" | "quit" | "exit" => {
            cmd_tx.send(DebugCommand::Exit)?;
        }
        _ => app.add_log(format!("Unknown command: {}", parts[0])),
    }
    Ok(())
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status Bar
            Constraint::Min(10),   // Logs and Context
            Constraint::Length(3), // Command Prompt
        ])
        .split(f.size());

    // Status Bar
    let is_running = app.running.load(Ordering::SeqCst);
    let status_style = if is_running {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let status_text = if let Some(ctx) = &app.cpu_context {
        format!(
            " [{}]  PC:${:04X}  HT:{:<3}  LED:{}  Motor:{}",
            if is_running { "RUNNING" } else { "PAUSED " },
            ctx.pc,
            ctx.ht,
            if ctx.led { "ON " } else { "OFF" },
            if ctx.motor { "ON " } else { "OFF" }
        )
    } else {
        format!(" [{}]  Initializing...", if is_running { "RUNNING" } else { "PAUSED " })
    };

    let status_bar = Paragraph::new(status_text)
        .style(status_style)
        .block(Block::default().borders(Borders::ALL).title("Drive Status"));
    f.render_widget(status_bar, chunks[0]);

    // Middle Area: Logs (Left) and Context (Right)
    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Logs
    let items: Vec<ListItem> = app.log.iter().map(|i| ListItem::new(i.as_str())).collect();
    let logs = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Events / History"))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    f.render_stateful_widget(logs, middle_chunks[0], &mut app.log_state);

    // Context / Disasm
    let mut context_lines = Vec::new();
    if let Some(ctx) = &app.cpu_context {
        let p = ctx.p;
        let flags = format!(
            "{}{}-{}{}{}{}{}",
            if p & 0x80 != 0 { 'N' } else { 'n' },
            if p & 0x40 != 0 { 'V' } else { 'v' },
            if p & 0x10 != 0 { 'B' } else { 'b' },
            if p & 0x08 != 0 { 'D' } else { 'd' },
            if p & 0x04 != 0 { 'I' } else { 'i' },
            if p & 0x02 != 0 { 'Z' } else { 'z' },
            if p & 0x01 != 0 { 'C' } else { 'c' },
        );

        context_lines.push(Line::from(vec![
            Span::styled("Registers: ", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        context_lines.push(Line::from(format!("  A:${:02X} X:${:02X} Y:${:02X} S:${:02X}", ctx.a, ctx.x, ctx.y, ctx.s)));
        context_lines.push(Line::from(format!("  P:${:02X} [{}]", ctx.p, flags)));
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![
            Span::styled("Instruction Trace: ", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        context_lines.push(Line::from(format!("  Processed: {}", ctx.disasm_prev)));
        context_lines.push(Line::from(vec![
            Span::styled(format!("  Next:      {}", ctx.disasm_next), Style::default().fg(Color::Cyan)),
        ]));
    }

    if let Some((addr, data)) = &app.memory_dump {
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![
            Span::styled(format!("Memory Dump at ${:04X}:", addr), Style::default().add_modifier(Modifier::BOLD)),
        ]));
        for (i, chunk) in data.chunks(8).enumerate() {
            let mut line_str = format!("  ${:04X}: ", addr.wrapping_add(i as u16 * 8));
            for b in chunk {
                line_str.push_str(&format!("{:02X} ", b));
            }
            context_lines.push(Line::from(line_str));
        }
    }

    let context_box = Paragraph::new(context_lines)
        .block(Block::default().borders(Borders::ALL).title("CPU Context"));
    f.render_widget(context_box, middle_chunks[1]);

    // Command Prompt
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().borders(Borders::ALL).title("Command Prompt"));
    f.render_widget(input, chunks[2]);
}

fn parse_addr(s: &str) -> Result<u16> {
    if s.starts_with("0x") {
        Ok(u16::from_str_radix(&s[2..], 16)?)
    } else if s.starts_with('$') {
        Ok(u16::from_str_radix(&s[1..], 16)?)
    } else {
        Ok(s.parse()?)
    }
}

fn get_cpu_context<H: DriveHardware>(drive: &Emulator<H>) -> CpuContext {
    let (disasm_next, _) = disasm::disassemble_at(drive.pc(), |addr| drive.bus().peek(addr));
    let disasm_prev = disasm::disassemble(&drive.cpu).to_string();

    CpuContext {
        pc: drive.pc(),
        a: drive.cpu.a,
        x: drive.cpu.x,
        y: drive.cpu.y,
        s: drive.cpu.s,
        p: drive.cpu.p,
        ht: drive.half_track(),
        led: drive.bus().led_on(),
        motor: drive.bus().motor_on(),
        disasm_prev,
        disasm_next: disasm_next.to_string(),
    }
}

fn emulation_loop(
    cmd_rx: Receiver<DebugCommand>,
    resp_tx: Sender<DebugResponse>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut rom_array = [0u8; 16384];
    rom_array.copy_from_slice(ROM_BYTES);

    static TRACK: ActiveTrack = ActiveTrack::new();
    static mut COMMAND_QUEUE: heapless::spsc::Queue<EmuCommand, 16> = heapless::spsc::Queue::new();

    let (cmd_tx_emu, mut cmd_rx_emu) = unsafe { (*core::ptr::addr_of_mut!(COMMAND_QUEUE)).split() };
    let bridge = HardwareBridge::new(&TRACK, cmd_tx_emu);
    let mut drive = Emulator::new(rom_array, bridge);
    drive.reset();

    let mut breakpoints: HashSet<u16> = HashSet::new();
    let mut last_tick = Instant::now();
    let mut cycle_accumulator = 0.0;
    let cycles_per_sec = 1_000_000.0;

    // Local cached states to detect changes
    let mut last_motor = false;
    let mut last_led = false;
    let mut last_ht = 0;

    loop {
        // Handle incoming commands
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
                    resp_tx.send(DebugResponse::Context(get_cpu_context(&drive)))?;
                }
                DebugCommand::Step => {
                    drive.step();
                    let ctx = get_cpu_context(&drive);
                    resp_tx.send(DebugResponse::DriveEvent(format!("Step: {}", ctx.disasm_prev)))?;
                    resp_tx.send(DebugResponse::Context(ctx))?;
                }
                DebugCommand::Reset => {
                    drive.reset();
                    resp_tx.send(DebugResponse::Context(get_cpu_context(&drive)))?;
                }
                DebugCommand::SetBreakpoint(addr) => {
                    breakpoints.insert(addr);
                }
                DebugCommand::ClearBreakpoint(addr) => {
                    breakpoints.remove(&addr);
                }
                DebugCommand::ReadMemory(addr, len) => {
                    let mut data = Vec::with_capacity(len as usize);
                    for i in 0..len {
                        data.push(drive.bus().peek(addr.wrapping_add(i)));
                    }
                    resp_tx.send(DebugResponse::Memory(addr, data))?;
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

                // Check for instruction boundary
                if drive.cpu.state == CycleState::FetchOpcode && drive.cpu.step_index == 0 {
                    if breakpoints.contains(&drive.pc()) {
                        running.store(false, Ordering::SeqCst);
                        resp_tx.send(DebugResponse::BreakpointHit(get_cpu_context(&drive)))?;
                        break;
                    }
                }
            }

            // Detect Drive Events
            let motor = drive.bus().motor_on();
            if motor != last_motor {
                resp_tx.send(DebugResponse::DriveEvent(format!("Motor: {}", if motor { "ON" } else { "OFF" })))?;
                last_motor = motor;
            }
            let led = drive.bus().led_on();
            if led != last_led {
                resp_tx.send(DebugResponse::DriveEvent(format!("LED: {}", if led { "ON" } else { "OFF" })))?;
                last_led = led;
            }
            let ht = drive.half_track();
            if ht != last_ht {
                resp_tx.send(DebugResponse::DriveEvent(format!("Head: HT={}", ht)))?;
                last_ht = ht;
            }

            // Drain hardware command queue from emu
            while let Some(cmd) = cmd_rx_emu.dequeue() {
                match cmd {
                    EmuCommand::StepHead(ht) => {
                         log::debug!("StepHead command: {}", ht);
                    }
                    _ => log::debug!("Hardware event: {:?}", cmd),
                }
            }

            // Sleep a bit if we are ahead
            if cycles_to_run == 0 {
                thread::sleep(Duration::from_micros(100));
            }
        } else {
            // If not running, just wait for next command
            thread::sleep(Duration::from_millis(10));
        }
    }
}
