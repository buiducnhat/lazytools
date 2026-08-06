use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use lazytools_core::registry::Registry;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind,
};
use ratatui::crossterm::execute;

use crate::app::App;

/// Poll interval. Uses a timeout rather than blocking indefinitely on `read()`,
/// so Phase 2B's debounce can fire without the user pressing another key.
const TICK: Duration = Duration::from_millis(16);

/// Runs the TUI with default key bindings.
pub fn run(registry: Registry) -> Result<()> {
    run_with(App::new(registry))
}

/// Runs the TUI with key bindings read from `~/.config/lazytools/keys.toml`.
pub fn run_with_user_config(registry: Registry) -> Result<()> {
    run_with(App::from_user_config(registry))
}

fn run_with(mut app: App) -> Result<()> {
    // `ratatui::init()` already installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableBracketedPaste)?;

    let result = run_loop(&mut terminal, &mut app);

    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();

    // After the terminal is back: there is no popup to show this in, and a
    // session that failed to save must not be a silent failure either.
    if let Err(e) = app.persist_session() {
        eprintln!("lazytools: couldn't save the session: {e}");
    }
    result
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        if app.needs_redraw() {
            let mut draw_err = None;
            terminal.draw(|f| {
                if let Err(e) = app.draw(f) {
                    draw_err = Some(e);
                }
            })?;
            if let Some(e) = draw_err {
                return Err(e);
            }
        }

        if event::poll(TICK)? {
            let ev = event::read()?;
            // Windows sends both Press and Release; only handle Press to avoid duplicates.
            let skip = matches!(&ev, Event::Key(k) if k.kind != KeyEventKind::Press);
            if !skip {
                app.event(&ev)?;
            }
        }

        app.process_queue()?;
        app.tick();

        if app.should_quit() {
            return Ok(());
        }
    }
}
