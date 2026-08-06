use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::key_list::KeysList;

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyConfig {
    pub keys: KeysList,
}

/// Matches the key the user pressed against a binding. Used in every component so
/// no `KeyCode::Char(..)` is scattered outside `KeysList`.
pub fn key_match(ev: &KeyEvent, binding: KeyEvent) -> bool {
    ev.code == binding.code && ev.modifiers == binding.modifiers
}

/// Why the config didn't work out as expected — lets `App` show a popup, **not** block startup.
#[derive(Debug)]
pub enum KeyConfigIssue {
    /// File has invalid TOML syntax.
    Malformed { path: PathBuf, msg: String },
    /// Some key names weren't recognized; those keys keep their defaults.
    UnknownKeys { path: PathBuf, entries: Vec<String> },
}

impl KeyConfigIssue {
    pub fn message(&self) -> String {
        match self {
            Self::Malformed { path, msg } => format!(
                "Could not read {}:\n{msg}\n\nUsing default keys.",
                path.display()
            ),
            Self::UnknownKeys { path, entries } => format!(
                "Skipping {} entries in {}:\n{}\n\nThe remaining keys still apply.",
                entries.len(),
                path.display(),
                entries.join("\n")
            ),
        }
    }
}

/// `~/.config/lazytools/keys.toml`. Returns `None` if HOME can't be determined.
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

/// Parses a string like `"ctrl+shift+p"` into a `KeyEvent`.
pub fn parse_key(spec: &str) -> Option<KeyEvent> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::NONE;
    let mut parts: Vec<&str> = spec.split('+').collect();
    // The last part is the key name, the earlier parts are modifiers. (The `+`
    // character itself can't be used as a binding — not needed yet, and it keeps
    // the parser simple.)
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
                // `shift` on a character is already baked into the uppercase letter itself.
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
    /// Reads the config from disk.
    ///
    /// **A broken config must never block app startup** — the user must always be
    /// able to get into the app to fix it. Three cases: no file (silent, the most
    /// common path), invalid TOML syntax (use defaults + report), unknown key
    /// name (keep the default for that specific key + report).
    pub fn load() -> (Self, Option<KeyConfigIssue>) {
        match default_path() {
            Some(path) => Self::load_from(&path),
            None => (Self::default(), None),
        }
    }

    pub fn load_from(path: &Path) -> (Self, Option<KeyConfigIssue>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            // No file (or unreadable) is a normal case, not an error.
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
                Some(_) => unknown.push(format!("  {name}: no such key binding")),
                None => unknown.push(format!(
                    "  {name} = \"{spec}\": unrecognized key combination"
                )),
            }
        }

        let issue = (!unknown.is_empty()).then(|| KeyConfigIssue::UnknownKeys {
            path: path.to_path_buf(),
            entries: unknown,
        });
        (config, issue)
    }

    /// `false` if no binding has that name.
    fn set_binding(&mut self, name: &str, ev: KeyEvent) -> bool {
        let k = &mut self.keys;
        match name {
            "quit" => k.quit = ev,
            "exit_popup" => k.exit_popup = ev,
            "focus_next" => k.focus_next = ev,
            "focus_prev" => k.focus_prev = ev,
            "focus_sidebar" => k.focus_sidebar = ev,
            "move_down" => k.move_down = ev,
            "move_up" => k.move_up = ev,
            "move_left" => k.move_left = ev,
            "move_right" => k.move_right = ev,
            "confirm" => k.confirm = ev,
            "run" => k.run = ev,
            "palette" => k.palette = ev,
            "theme" => k.theme = ev,
            "help" => k.help = ev,
            "copy" => k.copy = ev,
            "open_file" => k.open_file = ev,
            "save_file" => k.save_file = ev,
            "backspace" => k.backspace = ev,
            "delete" => k.delete = ev,
            "line_start" => k.line_start = ev,
            "line_end" => k.line_end = ev,
            "delete_to_start" => k.delete_to_start = ev,
            _ => return false,
        }
        true
    }

    /// Display string for a key: `^P`, `Tab`, `q`, `↑`.
    /// Both the cmdbar and the help popup read from here, so the hint never
    /// drifts from the real key binding.
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
            // Convention: display Ctrl combos with an uppercase letter: `^P`.
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
        // `shift` on a character is merged into the uppercase letter itself.
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
        let (config, issue) = KeyConfig::load_from(Path::new("/does/not/exist/keys.toml"));
        assert!(issue.is_none(), "a missing file is the normal path");
        assert_eq!(config.keys.quit, KeysList::default().quit);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("lazytools-test-malformed");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.toml");
        std::fs::write(&path, "this is not toml === {{{").unwrap();

        let (config, issue) = KeyConfig::load_from(&path);
        assert!(matches!(issue, Some(KeyConfigIssue::Malformed { .. })));
        assert_eq!(config.keys.quit, KeysList::default().quit);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A binding missing from `set_binding` fails *silently* — the field exists on
    /// `KeysList`, so the app works, but overriding it reports "no such key binding".
    /// `run` is new, so it gets an explicit guard against exactly that.
    #[test]
    fn the_run_key_is_overridable() {
        let dir = std::env::temp_dir().join("lazytools-test-run-key");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.toml");
        std::fs::write(&path, "run = \"f5\"\n").unwrap();

        let (config, issue) = KeyConfig::load_from(&path);
        assert!(
            issue.is_none(),
            "`run` must be a recognized binding name: {issue:?}"
        );
        assert_eq!(config.keys.run, ev(KeyCode::F(5), KeyModifiers::NONE));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_names_are_skipped_but_others_apply() {
        let dir = std::env::temp_dir().join("lazytools-test-unknown");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("keys.toml");
        std::fs::write(
            &path,
            "palette = \"ctrl+k\"\nno_such_key = \"x\"\nhelp = \"not+understood\"\n",
        )
        .unwrap();

        let (config, issue) = KeyConfig::load_from(&path);
        // Valid keys still get applied...
        assert_eq!(
            config.keys.palette,
            ev(KeyCode::Char('k'), KeyModifiers::CONTROL)
        );
        // ...unknown keys keep their default...
        assert_eq!(config.keys.help, KeysList::default().help);
        // ...and the user is informed about both skipped entries.
        match issue {
            Some(KeyConfigIssue::UnknownKeys { entries, .. }) => assert_eq!(entries.len(), 2),
            other => panic!("expected UnknownKeys, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
