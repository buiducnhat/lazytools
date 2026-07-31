//! Mọi phím của app sống ở đây, không rải rác trong logic component.
//! Đọc từ TOML được thêm ở Phase 03; 2A chỉ cần struct + `Default`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
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
    pub confirm: KeyEvent,
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
            confirm: key(KeyCode::Enter),
        }
    }
}
