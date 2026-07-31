pub mod key_config;
pub mod key_list;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use key_config::{KeyConfig, key_match};

/// The character the user just typed, if this is a plain character key.
///
/// Text input is **not** a binding, so it doesn't live in `KeysList`.
/// This function lives in `keys/` so that `KeyCode::Char` never leaks into
/// `components/` — preserving the invariant that "a component doesn't know
/// specific key codes".
/// `SHIFT` is passed through (uppercase); `CONTROL`/`ALT` are not (those are bindings).
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
