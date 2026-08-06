//! The built-in themes.
//!
//! Each is a straight transcription of a published palette — the colors are
//! the project's own, not an interpretation — mapped onto the nine slots of
//! [`Theme`]. Two conventions hold throughout, because a theme that only looks
//! right in a screenshot is worse than none:
//!
//! - **`terminal` is the default and sets nothing absolute.** It is built from
//!   the sixteen named ANSI colors, so it follows whatever the user's terminal
//!   already does, light or dark. Every other preset states exact colors, and
//!   therefore stops following it.
//! - **A preset that names a `background` must name every foreground too.**
//!   Half a theme — dark text painted on the terminal's own dark background —
//!   is the one failure mode that makes the app unusable rather than ugly.

use ratatui::style::Color;

use super::Theme;

pub struct Preset {
    /// What `config.toml` and the saved pick refer to. Kebab-case, stable.
    pub id: &'static str,
    /// What the picker shows.
    pub name: &'static str,
    pub theme: Theme,
}

/// The preset used when nothing has been chosen.
pub const DEFAULT_ID: &str = "terminal";

const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Ordered as the picker lists them: the terminal default first, then dark
/// themes, then the light ones.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: DEFAULT_ID,
        name: "Terminal (default)",
        // Deliberately the named ANSI colors — see the module docs.
        theme: Theme {
            background: Color::Reset,
            border: Color::DarkGray,
            border_focus: Color::Cyan,
            text: Color::Reset,
            text_dim: Color::DarkGray,
            error: Color::Red,
            selection_fg: Color::Black,
            selection_bg: Color::Cyan,
            title: Color::Cyan,
        },
    },
    Preset {
        id: "dracula",
        name: "Dracula",
        theme: Theme {
            background: rgb(0x282a36),
            border: rgb(0x44475a),
            border_focus: rgb(0xbd93f9),
            text: rgb(0xf8f8f2),
            text_dim: rgb(0x6272a4),
            error: rgb(0xff5555),
            selection_fg: rgb(0x282a36),
            selection_bg: rgb(0xbd93f9),
            title: rgb(0xff79c6),
        },
    },
    Preset {
        id: "nord",
        name: "Nord",
        theme: Theme {
            background: rgb(0x2e3440),
            border: rgb(0x434c5e),
            border_focus: rgb(0x88c0d0),
            text: rgb(0xd8dee9),
            text_dim: rgb(0x4c566a),
            error: rgb(0xbf616a),
            selection_fg: rgb(0x2e3440),
            selection_bg: rgb(0x88c0d0),
            title: rgb(0x81a1c1),
        },
    },
    Preset {
        id: "gruvbox-dark",
        name: "Gruvbox Dark",
        theme: Theme {
            background: rgb(0x282828),
            border: rgb(0x504945),
            border_focus: rgb(0xfabd2f),
            text: rgb(0xebdbb2),
            text_dim: rgb(0x928374),
            error: rgb(0xfb4934),
            selection_fg: rgb(0x282828),
            selection_bg: rgb(0xfabd2f),
            title: rgb(0x8ec07c),
        },
    },
    Preset {
        id: "solarized-dark",
        name: "Solarized Dark",
        theme: Theme {
            background: rgb(0x002b36),
            border: rgb(0x073642),
            border_focus: rgb(0x268bd2),
            text: rgb(0x93a1a1),
            text_dim: rgb(0x586e75),
            error: rgb(0xdc322f),
            selection_fg: rgb(0x002b36),
            selection_bg: rgb(0x2aa198),
            title: rgb(0xb58900),
        },
    },
    Preset {
        id: "catppuccin-mocha",
        name: "Catppuccin Mocha",
        theme: Theme {
            background: rgb(0x1e1e2e),
            border: rgb(0x45475a),
            border_focus: rgb(0xcba6f7),
            text: rgb(0xcdd6f4),
            text_dim: rgb(0x6c7086),
            error: rgb(0xf38ba8),
            selection_fg: rgb(0x1e1e2e),
            selection_bg: rgb(0xcba6f7),
            title: rgb(0x89b4fa),
        },
    },
    Preset {
        id: "tokyo-night",
        name: "Tokyo Night",
        theme: Theme {
            background: rgb(0x1a1b26),
            border: rgb(0x3b4261),
            border_focus: rgb(0x7aa2f7),
            text: rgb(0xc0caf5),
            text_dim: rgb(0x565f89),
            error: rgb(0xf7768e),
            selection_fg: rgb(0x1a1b26),
            selection_bg: rgb(0x7aa2f7),
            title: rgb(0xbb9af7),
        },
    },
    Preset {
        id: "one-dark",
        name: "One Dark",
        theme: Theme {
            background: rgb(0x282c34),
            border: rgb(0x3e4451),
            border_focus: rgb(0x61afef),
            text: rgb(0xabb2bf),
            text_dim: rgb(0x5c6370),
            error: rgb(0xe06c75),
            selection_fg: rgb(0x282c34),
            selection_bg: rgb(0x61afef),
            title: rgb(0xc678dd),
        },
    },
    Preset {
        id: "monokai",
        name: "Monokai",
        theme: Theme {
            background: rgb(0x272822),
            border: rgb(0x49483e),
            border_focus: rgb(0xa6e22e),
            text: rgb(0xf8f8f2),
            text_dim: rgb(0x75715e),
            error: rgb(0xf92672),
            selection_fg: rgb(0x272822),
            selection_bg: rgb(0xa6e22e),
            title: rgb(0x66d9ef),
        },
    },
    Preset {
        id: "solarized-light",
        name: "Solarized Light",
        theme: Theme {
            background: rgb(0xfdf6e3),
            border: rgb(0xeee8d5),
            border_focus: rgb(0x268bd2),
            text: rgb(0x586e75),
            text_dim: rgb(0x93a1a1),
            error: rgb(0xdc322f),
            selection_fg: rgb(0xfdf6e3),
            selection_bg: rgb(0x268bd2),
            title: rgb(0xb58900),
        },
    },
    Preset {
        id: "github-light",
        name: "GitHub Light",
        theme: Theme {
            background: rgb(0xffffff),
            border: rgb(0xd0d7de),
            border_focus: rgb(0x0969da),
            text: rgb(0x24292f),
            text_dim: rgb(0x57606a),
            error: rgb(0xcf222e),
            selection_fg: rgb(0xffffff),
            selection_bg: rgb(0x0969da),
            title: rgb(0x8250df),
        },
    },
];

pub fn find(id: &str) -> Option<&'static Preset> {
    let id = id.trim().to_ascii_lowercase();
    PRESETS.iter().find(|p| p.id == id)
}

/// Position of a preset in `PRESETS` — where the picker opens its cursor.
pub fn index_of(id: &str) -> Option<usize> {
    let id = id.trim().to_ascii_lowercase();
    PRESETS.iter().position(|p| p.id == id)
}

/// Every id, comma-separated. Only used to make an error message name the
/// alternatives instead of leaving the user to guess.
pub fn id_list() -> String {
    PRESETS.iter().map(|p| p.id).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_kebab_case() {
        let mut seen = Vec::new();
        for preset in PRESETS {
            assert!(!seen.contains(&preset.id), "duplicate id: {}", preset.id);
            assert!(
                preset
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} must be kebab-case — it is written in config.toml",
                preset.id
            );
            seen.push(preset.id);
        }
    }

    #[test]
    fn the_default_exists_and_is_the_terminal_following_one() {
        let preset = find(DEFAULT_ID).expect("the default must be in the list");
        assert_eq!(preset.theme, Theme::default());
        assert_eq!(index_of(DEFAULT_ID), Some(0), "it is listed first");
    }

    /// The rule from the module docs, checked rather than trusted: a preset
    /// that paints its own background must paint every foreground too, or it
    /// leaves the app with the terminal's text color on its own background.
    #[test]
    fn no_preset_mixes_an_absolute_background_with_an_inherited_foreground() {
        for preset in PRESETS {
            let t = &preset.theme;
            if t.background == Color::Reset {
                continue;
            }
            for (slot, color) in [
                ("text", t.text),
                ("text_dim", t.text_dim),
                ("border", t.border),
                ("border_focus", t.border_focus),
                ("error", t.error),
                ("title", t.title),
                ("selection_fg", t.selection_fg),
                ("selection_bg", t.selection_bg),
            ] {
                assert_ne!(
                    color,
                    Color::Reset,
                    "{}: {slot} inherits the terminal's color on top of an explicit background",
                    preset.id
                );
            }
        }
    }

    /// Selected text has to stay readable: the highlight's own foreground must
    /// never be the same color as the bar it is drawn on.
    #[test]
    fn selection_foreground_and_background_always_differ() {
        for preset in PRESETS {
            assert_ne!(
                preset.theme.selection_fg, preset.theme.selection_bg,
                "{}: the selected row would be invisible",
                preset.id
            );
        }
    }

    #[test]
    fn lookup_is_forgiving_about_case_and_surrounding_space() {
        assert_eq!(find(" Dracula ").map(|p| p.id), Some("dracula"));
        assert!(find("nosuchtheme").is_none());
        assert!(id_list().contains("nord"), "{}", id_list());
    }
}
