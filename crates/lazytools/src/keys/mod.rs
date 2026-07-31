pub mod key_config;
pub mod key_list;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use key_config::{KeyConfig, key_match};

/// Ký tự người dùng vừa gõ, nếu đây là phím ký tự thường.
///
/// Nhập text **không** phải một binding, nên nó không nằm trong `KeysList`.
/// Hàm này sống ở `keys/` để `KeyCode::Char` không rò rỉ vào `components/` —
/// giữ đúng bất biến "component không biết mã phím cụ thể".
/// `SHIFT` được cho qua (chữ hoa); `CONTROL`/`ALT` thì không (đó là binding).
pub fn typed_char(ev: &KeyEvent) -> Option<char> {
    if ev
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    match ev.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}
