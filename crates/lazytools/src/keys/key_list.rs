//! Mọi phím của app sống ở đây, không rải rác trong logic component.
//! Đọc từ TOML được thêm ở Phase 03; 2A chỉ cần struct + `Default`.

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

    // Soạn thảo trong TextArea.
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

            backspace: key(KeyCode::Backspace),
            delete: key(KeyCode::Delete),
            line_start: key(KeyCode::Home),
            line_start_alt: ctrl(KeyCode::Char('a')),
            line_end: key(KeyCode::End),
            delete_to_start: ctrl(KeyCode::Char('u')),
        }
    }
}
