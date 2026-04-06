use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

mod audio;
mod music;
mod presets;
mod state;
mod ui;

use audio::engine::AudioEngine;
use state::AppState;
use ui::{app::App, render::render};

fn main() -> Result<()> {
    let state = AppState::new();

    // Audio engine – non-fatal; the UI still works without a sound card
    let _audio = match AudioEngine::new(Arc::clone(&state)) {
        Ok(eng) => {
            // Print device info before entering alternate screen
            eprintln!(
                "Audio: {} – {} Hz – {} ch",
                eng.device_name, eng.sample_rate, eng.channel_count
            );
            Some(eng)
        }
        Err(e) => {
            eprintln!("Audio unavailable: {e}");
            None
        }
    };

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend  = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;
    term.hide_cursor()?;

    let result = run_loop(&mut term, state);

    // Restore terminal unconditionally
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;

    result
}

fn run_loop(
    term:  &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<AppState>,
) -> Result<()> {
    let mut app           = App::new(state);
    let mut status_frames = 0u32;

    loop {
        term.draw(|f| render(f, &app))?;

        // Age-out status message after ~3 seconds (≈ 180 frames at 60 fps)
        if app.status_msg.is_some() {
            status_frames += 1;
            if status_frames > 180 {
                app.clear_status();
                status_frames = 0;
            }
        } else {
            status_frames = 0;
        }

        // Poll for events with 16 ms timeout → ~60 fps render loop
        if event::poll(Duration::from_millis(16))? {
            let ev = event::read()?;
            if !app.handle_event(ev) {
                break;
            }
        }
    }

    Ok(())
}
