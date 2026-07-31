use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// Vì sao config không được như ý — để `App` hiện popup, **không** để chặn khởi động.
#[derive(Debug)]
pub enum KeyConfigIssue {
    /// File sai cú pháp TOML.
    Malformed { path: PathBuf, msg: String },
    /// Một số tên phím không nhận diện được; các phím đó giữ mặc định.
    UnknownKeys { path: PathBuf, entries: Vec<String> },
}

impl KeyConfigIssue {
    pub fn message(&self) -> String {
        match self {
            Self::Malformed { path, msg } => format!(
                "Không đọc được {}:\n{msg}\n\nĐang dùng phím mặc định.",
                path.display()
            ),
            Self::UnknownKeys { path, entries } => format!(
                "Bỏ qua {} mục trong {}:\n{}\n\nCác phím còn lại vẫn được áp dụng.",
                entries.len(),
                path.display(),
                entries.join("\n")
            ),
        }
    }
}

/// `~/.config/lazytools/keys.toml`. Trả `None` nếu không xác định được HOME.
pub fn default_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())?;
    Some(
        Path::new(&home)
            .join(".config")
            .join("lazytools")
            .join("keys.toml"),
    )
}

/// Phân tích chuỗi dạng `"ctrl+shift+p"` thành `KeyEvent`.
pub fn parse_key(spec: &str) -> Option<KeyEvent> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut parts: Vec<&str> = spec.split('+').collect();
    // Phần cuối là tên phím, phần trước là modifier. (Bản thân dấu `+` không
    // dùng được làm binding — chưa cần tới, và giữ parser đơn giản.)
    let name = parts.pop()?;

    for m in &parts {
        match m.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "option" | "meta" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return None,
        }
    }

    let lower = name.trim().to_ascii_lowercase();
    let code = match lower.as_str() {
        "tab" => KeyCode::Tab,
        "backtab" | "shift+tab" => KeyCode::BackTab,
        "esc" | "escape" => KeyCode::Esc,
        "enter" | "return" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pgup" | "pageup" => KeyCode::PageUp,
        "pgdn" | "pagedown" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        other => {
            if let Some(n) = other
                .strip_prefix('f')
                .and_then(|d| d.parse::<u8>().ok())
                .filter(|n| (1..=12).contains(n))
            {
                KeyCode::F(n)
            } else {
                let mut chars = name.trim().chars();
                let c = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                // `shift` trên ký tự đã nằm sẵn trong chính chữ hoa.
                if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_alphabetic() {
                    modifiers.remove(KeyModifiers::SHIFT);
                    KeyCode::Char(c.to_ascii_uppercase())
                } else {
                    KeyCode::Char(c)
                }
            }
        }
    };

    Some(KeyEvent::new(code, modifiers))
}

impl KeyConfig {
    /// Đọc config từ đĩa.
    ///
    /// **Config hỏng không được chặn app khởi động** — người dùng phải luôn vào
    /// được app để sửa. Ba trường hợp: không có file (im lặng, đường đi phổ
    /// biến nhất), sai cú pháp TOML (dùng default + báo), tên phím lạ (giữ
    /// default cho riêng phím đó + báo).
    pub fn load() -> (Self, Option<KeyConfigIssue>) {
        match default_path() {
            Some(path) => Self::load_from(&path),
            None => (Self::default(), None),
        }
    }

    pub fn load_from(path: &Path) -> (Self, Option<KeyConfigIssue>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            // Không có file (hoặc không đọc được) là chuyện bình thường, không phải lỗi.
            return (Self::default(), None);
        };

        let table: BTreeMap<String, String> = match toml::from_str(&text) {
            Ok(t) => t,
            Err(e) => {
                return (
                    Self::default(),
                    Some(KeyConfigIssue::Malformed {
                        path: path.to_path_buf(),
                        msg: e.to_string(),
                    }),
                );
            }
        };

        let mut config = Self::default();
        let mut unknown = Vec::new();

        for (name, spec) in table {
            match parse_key(&spec) {
                Some(ev) if config.set_binding(&name, ev) => {}
                Some(_) => unknown.push(format!("  {name}: không có phím nào tên vậy")),
                None => unknown.push(format!("  {name} = \"{spec}\": không hiểu tổ hợp phím")),
            }
        }

        let issue = (!unknown.is_empty()).then(|| KeyConfigIssue::UnknownKeys {
            path: path.to_path_buf(),
            entries: unknown,
        });
        (config, issue)
    }

    /// `false` nếu không có binding nào mang tên đó.
    fn set_binding(&mut self, name: &str, ev: KeyEvent) -> bool {
        let k = &mut self.keys;
        match name {
            "quit" => k.quit = ev,
            "exit_popup" => k.exit_popup = ev,
            "focus_next" => k.focus_next = ev,
            "focus_prev" => k.focus_prev = ev,
            "move_down" => k.move_down = ev,
            "move_up" => k.move_up = ev,
            "move_left" => k.move_left = ev,
            "move_right" => k.move_right = ev,
            "confirm" => k.confirm = ev,
            "palette" => k.palette = ev,
            "help" => k.help = ev,
            "copy" => k.copy = ev,
            "backspace" => k.backspace = ev,
            "delete" => k.delete = ev,
            "line_start" => k.line_start = ev,
            "line_end" => k.line_end = ev,
            "delete_to_start" => k.delete_to_start = ev,
            _ => return false,
        }
        true
    }

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
            // Quy ước hiển thị tổ hợp Ctrl là chữ hoa: `^P`.
            KeyCode::Char(c) if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                s.push(c.to_ascii_uppercase());
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parses_plain_keys() {
        assert_eq!(
            parse_key("q"),
            Some(ev(KeyCode::Char('q'), KeyModifiers::NONE))
        );
        assert_eq!(parse_key("tab"), Some(ev(KeyCode::Tab, KeyModifiers::NONE)));
        assert_eq!(parse_key("Esc"), Some(ev(KeyCode::Esc, KeyModifiers::NONE)));
        assert_eq!(
            parse_key("space"),
            Some(ev(KeyCode::Char(' '), KeyModifiers::NONE))
        );
        assert_eq!(parse_key("f5"), Some(ev(KeyCode::F(5), KeyModifiers::NONE)));
        assert_eq!(parse_key("up"), Some(ev(KeyCode::Up, KeyModifiers::NONE)));
    }

    #[test]
    fn parses_modifiers() {
        assert_eq!(
            parse_key("ctrl+p"),
            Some(ev(KeyCode::Char('p'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            parse_key("alt+enter"),
            Some(ev(KeyCode::Enter, KeyModifiers::ALT))
        );
        // `shift` trên ký tự được gộp vào chính chữ hoa.
        assert_eq!(
            parse_key("ctrl+shift+p"),
            Some(ev(KeyCode::Char('P'), KeyModifiers::CONTROL))
        );
    }

    #[test]
    fn rejects_nonsense() {
        assert_eq!(parse_key(""), None);
        assert_eq!(parse_key("hyper+p"), None);
        assert_eq!(parse_key("notakey"), None);
        assert_eq!(parse_key("f99"), None);
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let (config, issue) = KeyConfig::load_from(Path::new("/khong/ton/tai/keys.toml"));
        assert!(
            issue.is_none(),
            "file không tồn tại là đường đi bình thường"
        );
        assert_eq!(config.keys.quit, KeysList::default().quit);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("lazytools-test-malformed");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.toml");
        std::fs::write(&path, "này thì không phải toml === {{{").unwrap();

        let (config, issue) = KeyConfig::load_from(&path);
        assert!(matches!(issue, Some(KeyConfigIssue::Malformed { .. })));
        assert_eq!(config.keys.quit, KeysList::default().quit);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_names_are_skipped_but_others_apply() {
        let dir = std::env::temp_dir().join("lazytools-test-unknown");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.toml");
        std::fs::write(
            &path,
            "palette = \"ctrl+k\"\nkhong_co_phim_nay = \"x\"\nhelp = \"khong+hieu\"\n",
        )
        .unwrap();

        let (config, issue) = KeyConfig::load_from(&path);
        // Phím hợp lệ vẫn được áp dụng...
        assert_eq!(
            config.keys.palette,
            ev(KeyCode::Char('k'), KeyModifiers::CONTROL)
        );
        // ...phím lạ giữ mặc định...
        assert_eq!(config.keys.help, KeysList::default().help);
        // ...và người dùng được báo về cả hai mục bị bỏ qua.
        match issue {
            Some(KeyConfigIssue::UnknownKeys { entries, .. }) => assert_eq!(entries.len(), 2),
            other => panic!("kỳ vọng UnknownKeys, nhận {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
