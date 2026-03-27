use anyhow::Result;
use crossbeam_channel::Sender;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::bridge::{CpuContext, DebugCommand};

pub struct App {
    pub log: Vec<String>,
    pub log_state: ListState,
    pub cpu_context: Option<CpuContext>,
    pub running: Arc<AtomicBool>,
    pub breakpoints: HashSet<u16>,
    pub input: String,
    pub memory_dump: Option<(u16, Vec<u8>)>,
}

impl App {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            log: vec!["Debugger started. Run the drive first (F5) before using IEC commands.".to_string()],
            log_state: ListState::default(),
            cpu_context: None,
            running,
            breakpoints: HashSet::new(),
            input: String::new(),
            memory_dump: None,
        }
    }

    pub fn add_log(&mut self, s: String) {
        for line in s.lines() {
            self.log.push(line.to_string());
        }
        if self.log.len() > 1000 {
            self.log.remove(0);
        }
        self.log_state.select(Some(self.log.len().saturating_sub(1)));
    }
}

pub fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

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
            ctx.pc, ctx.ht,
            if ctx.led { "ON " } else { "OFF" },
            if ctx.motor { "ON " } else { "OFF" },
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
            if p & 0x80 != 0 { 'N' } else { 'n' },
            if p & 0x40 != 0 { 'V' } else { 'v' },
            if p & 0x10 != 0 { 'B' } else { 'b' },
            if p & 0x08 != 0 { 'D' } else { 'd' },
            if p & 0x04 != 0 { 'I' } else { 'i' },
            if p & 0x02 != 0 { 'Z' } else { 'z' },
            if p & 0x01 != 0 { 'C' } else { 'c' },
        );

        context_lines.push(Line::from(vec![
            Span::styled("Registers: ", Style::default().add_modifier(Modifier::BOLD))
        ]));
        context_lines.push(Line::from(format!(
            "  A:${:02X} X:${:02X} Y:${:02X} S:${:02X}", ctx.a, ctx.x, ctx.y, ctx.s
        )));
        context_lines.push(Line::from(format!("  P:${:02X} [{}]", ctx.p, flags)));
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![
            Span::styled("Disassembly: ", Style::default().add_modifier(Modifier::BOLD))
        ]));

        for line in &ctx.disasm_window {
            let style = if line.addr == ctx.pc {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let prefix = if line.addr == ctx.pc { ">> " } else { "   " };
            context_lines.push(Line::from(vec![
                Span::styled(format!("{}{}", prefix, line.text), style)
            ]));
        }
    }

    if let Some((addr, data)) = &app.memory_dump {
        context_lines.push(Line::from(""));
        context_lines.push(Line::from(vec![
            Span::styled(
                format!("Memory Dump at ${:04X}:", addr),
                Style::default().add_modifier(Modifier::BOLD),
            )
        ]));
        for (i, chunk) in data.chunks(8).enumerate() {
            let mut line_str = format!("  ${:04X}: ", addr.wrapping_add(i as u16 * 8));
            for b in chunk { line_str.push_str(&format!("{:02X} ", b)); }
            context_lines.push(Line::from(line_str));
        }
    }

    let context_box = Paragraph::new(context_lines)
        .block(Block::default().borders(Borders::ALL).title("CPU Context"));
    f.render_widget(context_box, middle_chunks[1]);

    let input_text = format!("> {}", app.input);
    let input = Paragraph::new(input_text.as_str())
        .block(Block::default().borders(Borders::ALL)
            .title("Command  F5:Run/Pause  F6:Step  F7:Reset  Esc:Quit"));
    f.render_widget(input, chunks[2]);
    f.set_cursor_position((
        chunks[2].x + 1 + 2 + app.input.len() as u16, // border + "> " + typed text
        chunks[2].y + 1,
    ));
}

pub fn handle_command(line: String, app: &mut App, cmd_tx: &Sender<DebugCommand>) -> Result<()> {
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
        "dir" | "ls" => {
            app.add_log("Sending IEC dir request (drive must be running)...".to_string());
            cmd_tx.send(DebugCommand::IECCommand("LOAD:$".to_string()))?;
        }
        "status" | "st" => {
            app.add_log("Sending IEC status request (drive must be running)...".to_string());
            cmd_tx.send(DebugCommand::IECCommand("STATUS".to_string()))?;
        }
        "reset" => {
            cmd_tx.send(DebugCommand::Reset)?;
            app.add_log("Drive Reset.".to_string());
        }
        "help" | "h" => {
            app.add_log(
                "Commands: r(un), p(ause), s(tep), bp <addr>, m(em) <addr> [len], \
                 mount <path>, unmount, dir, status, reset, exit".to_string()
            );
        }
        "q" | "quit" | "exit" => { cmd_tx.send(DebugCommand::Exit)?; }
        _ => app.add_log(format!("Unknown command: {}", parts[0])),
    }
    Ok(())
}

pub fn parse_addr(s: &str) -> Result<u16> {
    if let Some(stripped) = s.strip_prefix("0x") {
        Ok(u16::from_str_radix(stripped, 16)?)
    } else if let Some(stripped) = s.strip_prefix('$') {
        Ok(u16::from_str_radix(stripped, 16)?)
    } else {
        Ok(s.parse()?)
    }
}
