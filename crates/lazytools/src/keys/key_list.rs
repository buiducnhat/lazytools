//! Every key binding in the app lives here, not scattered through component logic.
//! TOML loading was added in Phase 03; 2A only needed the struct + `Default`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

const fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

#[derive(Debug, Clone, Copy)]
pub struct KeysList {
    pub quit: KeyEvent,
    pub exit_popup: KeyEvent,
    pub focus_next: KeyEvent,
    pub focus_prev: KeyEvent,
    pub move_down: KeyEvent,
    pub move_up: KeyEvent,
    pub move_down_alt: KeyEvent,
    pub move_up_alt: KeyEvent,
    pub move_left: KeyEvent,
    pub move_right: KeyEvent,
    pub confirm: KeyEvent,
    /// Runs the tool from any field, including read-only outputs. `confirm` still
    /// runs it too, but only where that key isn't already spoken for — a multiline
    /// text field needs `Enter` for line breaks.
    pub run: KeyEvent,
    pub palette: KeyEvent,
    pub help: KeyEvent,
    pub copy: KeyEvent,
    pub open_file: KeyEvent,
    pub save_file: KeyEvent,

    // Editing within TextArea.
    pub backspace: KeyEvent,
    pub delete: KeyEvent,
    pub line_start: KeyEvent,
    pub line_start_alt: KeyEvent,
    pub line_end: KeyEvent,
    pub delete_to_start: KeyEvent,
}

impl Default for KeysList {
    fn default() -> Self {
        Self {
            quit: key(KeyCode::Char('q')),
            exit_popup: key(KeyCode::Esc),
            focus_next: key(KeyCode::Tab),
            focus_prev: key(KeyCode::BackTab),
            move_down: key(KeyCode::Char('j')),
            move_up: key(KeyCode::Char('k')),
            move_down_alt: key(KeyCode::Down),
            move_up_alt: key(KeyCode::Up),
            move_left: key(KeyCode::Left),
            move_right: key(KeyCode::Right),
            confirm: key(KeyCode::Enter),
            // Ctrl for the same reason as `open_file`/`save_file` below: a text input
            // consumes plain characters first. `Ctrl+Enter` would read better but many
            // terminals can't tell it apart from `Enter` without the Kitty keyboard
            // protocol, so it would silently do nothing for some users.
            run: ctrl(KeyCode::Char('r')),
            palette: ctrl(KeyCode::Char('p')),
            help: key(KeyCode::Char('?')),
            copy: key(KeyCode::Char('y')),
            // Ctrl rather than a bare `o`/`s`: a text input field consumes plain
            // characters first, so a bare key would be useless while typing.
            open_file: ctrl(KeyCode::Char('o')),
            save_file: ctrl(KeyCode::Char('s')),

            backspace: key(KeyCode::Backspace),
            delete: key(KeyCode::Delete),
            line_start: key(KeyCode::Home),
            line_start_alt: ctrl(KeyCode::Char('a')),
            line_end: key(KeyCode::End),
            delete_to_start: ctrl(KeyCode::Char('u')),
        }
    }
}
