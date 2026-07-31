use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::key_list::KeysList;

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyConfig {
    pub keys: KeysList,
}

/// So khớp phím người dùng bấm với một binding. Dùng ở mọi component để không
/// có `KeyCode::Char(..)` nào nằm rải rác ngoài `KeysList`.
pub fn key_match(ev: &KeyEvent, binding: KeyEvent) -> bool {
    ev.code == binding.code && ev.modifiers == binding.modifiers
}

impl KeyConfig {
    /// Chuỗi hiển thị của một phím: `^P`, `Tab`, `q`, `↑`.
    /// cmdbar và help popup đều đọc từ đây nên hint không bao giờ lệch phím thật.
    pub fn hint(&self, ev: KeyEvent) -> String {
        let mut s = String::new();
        if ev.modifiers.contains(KeyModifiers::CONTROL) {
            s.push('^');
        }
        if ev.modifiers.contains(KeyModifiers::ALT) {
            s.push('⌥');
        }
        match ev.code {
            KeyCode::Char(' ') => s.push_str("Space"),
            KeyCode::Char(c) => s.push(c),
            KeyCode::Tab => s.push_str("Tab"),
            KeyCode::BackTab => s.push_str("⇧Tab"),
            KeyCode::Enter => s.push_str("Enter"),
            KeyCode::Esc => s.push_str("Esc"),
            KeyCode::Backspace => s.push('⌫'),
            KeyCode::Delete => s.push_str("Del"),
            KeyCode::Up => s.push('↑'),
            KeyCode::Down => s.push('↓'),
            KeyCode::Left => s.push('←'),
            KeyCode::Right => s.push('→'),
            KeyCode::Home => s.push_str("Home"),
            KeyCode::End => s.push_str("End"),
            KeyCode::PageUp => s.push_str("PgUp"),
            KeyCode::PageDown => s.push_str("PgDn"),
            KeyCode::F(n) => s.push_str(&format!("F{n}")),
            other => s.push_str(&format!("{other:?}")),
        }
        s
    }
}
