mod app;
mod bridge;
mod emulation;
mod iec;

use anyhow::Result;
use crossbeam_channel::unbounded;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::task::LocalSet;

use app::{handle_command, ui, App};
use bridge::{DebugCommand, DebugResponse};
use emulation::emulation_loop;

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

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
                DebugResponse::Context(ctx)       => { app.cpu_context = Some(ctx); }
                DebugResponse::BreakpointHit(ctx) => {
                    app.add_log(format!("Breakpoint hit at ${:04X}", ctx.pc));
                    app.cpu_context = Some(ctx);
                }
                DebugResponse::DriveEvent(s)      => { app.add_log(s); }
                DebugResponse::Memory(addr, data) => { app.memory_dump = Some((addr, data)); }
                DebugResponse::IECResponse(s)     => { app.add_log(format!("[IEC] {}", s)); }
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
