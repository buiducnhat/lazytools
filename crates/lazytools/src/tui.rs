use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use lazytools_core::registry::Registry;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind,
};
use ratatui::crossterm::execute;

use crate::app::App;

/// Nhịp poll. Có timeout chứ không `read()` chặn vô hạn, để debounce của
/// Phase 2B kích hoạt được mà không cần người dùng bấm thêm phím.
const TICK: Duration = Duration::from_millis(16);

/// Chạy TUI với phím mặc định.
pub fn run(registry: Registry) -> Result<()> {
    run_with(App::new(registry))
}

/// Chạy TUI với phím đọc từ `~/.config/lazytools/keys.toml`.
pub fn run_with_user_config(registry: Registry) -> Result<()> {
    run_with(App::from_user_config(registry))
}

fn run_with(app: App) -> Result<()> {
    // `ratatui::init()` đã tự cài panic hook khôi phục terminal.
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableBracketedPaste)?;

    let result = run_loop(&mut terminal, app);

    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, mut app: App) -> Result<()> {
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
            // Windows gửi cả Press lẫn Release; chỉ xử lý Press để không nhân đôi.
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
