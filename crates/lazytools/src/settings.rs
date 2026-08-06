//! `~/.config/lazytools/config.toml` — the hand-written settings file.
//!
//! Key bindings keep their own `keys.toml`; this file holds everything else
//! (`[session]`, `[theme]`).
//!
//! **A broken settings file must never block startup** — the same rule
//! `keys.toml` follows, and for the same reason: the user has to be able to get
//! into the app to fix it. Recovery is per entry wherever the file parses at
//! all, so one bad color doesn't cost the user their restore mode.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ratatui::style::Color;
use serde::Deserialize;

use crate::ui::{Theme, parse_color, themes};

/// How much of the previous session comes back on startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Restore {
    /// Nothing is written to disk and nothing is restored.
    Off,
    /// The last open tool and the values of its **options**. The default:
    /// options are how you like a tool configured, while an input is the data
    /// you were working on — often the very token or payload you pasted in to
    /// decode, which a utility has no business keeping.
    #[default]
    Options,
    /// Also the input fields. Never `Secret` ones, in any mode.
    All,
}

impl Restore {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" => Some(Self::Off),
            "options" => Some(Self::Options),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn is_off(self) -> bool {
        self == Self::Off
    }

    pub fn includes_inputs(self) -> bool {
        self == Self::All
    }
}

#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub restore: Restore,
    /// The preset named by `[theme] name`, already checked to exist. `None`
    /// when the file names none, which is also what lets a pick made in the
    /// app be recognized as still current — see `theme_state`.
    pub theme_name: Option<String>,
    /// Per-color entries from `[theme]`, kept separately from the resolved
    /// theme because they outlive it: whichever preset the picker moves to,
    /// these still apply on top.
    pub theme_overrides: Vec<(String, Color)>,
    /// The theme to start with — `theme_name`'s preset with the overrides
    /// applied.
    pub theme: Theme,
}

impl Settings {
    /// The named preset with this file's color overrides on top. The picker
    /// calls this for every theme it previews, which is why the overrides are
    /// stored rather than folded into `theme` once.
    pub fn theme_for(&self, preset_id: &str) -> Theme {
        let mut theme = themes::find(preset_id).map_or_else(Theme::default, |p| p.theme);
        for (slot, color) in &self.theme_overrides {
            theme.set_color(slot, *color);
        }
        theme
    }
}

/// Why the settings file didn't fully apply. Reported, never fatal.
#[derive(Debug)]
pub enum SettingsIssue {
    /// The file isn't valid TOML, or a section isn't the shape it must be —
    /// nothing in it could be used.
    Malformed { path: PathBuf, msg: String },
    /// Individual entries weren't understood; everything else still applies.
    Skipped { path: PathBuf, entries: Vec<String> },
}

impl SettingsIssue {
    pub fn message(&self) -> String {
        match self {
            Self::Malformed { path, msg } => format!(
                "Could not read {}:\n{msg}\n\nUsing default settings.",
                path.display()
            ),
            Self::Skipped { path, entries } => format!(
                "Skipping {} entries in {}:\n{}\n\nThe remaining settings still apply.",
                entries.len(),
                path.display(),
                entries.join("\n")
            ),
        }
    }
}

/// `deny_unknown_fields` on the sections a typo can hide in. `[theme]` is a
/// plain map instead, so an unrecognized color name is reported entry by entry
/// the way `keys.toml` reports an unrecognized binding.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    #[serde(default)]
    session: RawSession,
    #[serde(default)]
    theme: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSession {
    restore: Option<String>,
}

pub const FILE_NAME: &str = "config.toml";

/// The one key in `[theme]` that names a preset rather than a color.
const NAME_KEY: &str = "name";

impl Settings {
    pub fn load() -> (Self, Option<SettingsIssue>) {
        match crate::paths::config_file(FILE_NAME) {
            Some(path) => Self::load_from(&path),
            None => (Self::default(), None),
        }
    }

    pub fn load_from(path: &Path) -> (Self, Option<SettingsIssue>) {
        let Ok(text) = std::fs::read_to_string(path) else {
            // No file (or unreadable) is the normal case, not an error.
            return (Self::default(), None);
        };

        let raw: RawSettings = match toml::from_str(&text) {
            Ok(raw) => raw,
            Err(e) => {
                return (
                    Self::default(),
                    Some(SettingsIssue::Malformed {
                        path: path.to_path_buf(),
                        msg: e.to_string(),
                    }),
                );
            }
        };

        let mut settings = Self::default();
        let mut skipped = Vec::new();

        if let Some(spec) = &raw.session.restore {
            match Restore::parse(spec) {
                Some(r) => settings.restore = r,
                None => skipped.push(format!(
                    "  session.restore = \"{spec}\": expected \"off\", \"options\", or \"all\""
                )),
            }
        }

        for (name, spec) in &raw.theme {
            if name == NAME_KEY {
                match themes::find(spec) {
                    Some(preset) => settings.theme_name = Some(preset.id.to_string()),
                    None => skipped.push(format!(
                        "  theme.name = \"{spec}\": no such theme (have: {})",
                        themes::id_list()
                    )),
                }
                continue;
            }
            match parse_color(spec) {
                // Checked against a scratch theme so an unknown slot is
                // reported here rather than discovered on every later preview.
                Some(color) if Theme::default().set_color(name, color) => {
                    settings.theme_overrides.push((name.clone(), color));
                }
                Some(_) => skipped.push(format!("  theme.{name}: no such color in the theme")),
                None => skipped.push(format!(
                    "  theme.{name} = \"{spec}\": not a color name, #rrggbb, or 0-255"
                )),
            }
        }

        settings.theme =
            settings.theme_for(settings.theme_name.as_deref().unwrap_or(themes::DEFAULT_ID));

        let issue = (!skipped.is_empty()).then(|| SettingsIssue::Skipped {
            path: path.to_path_buf(),
            entries: skipped,
        });
        (settings, issue)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;

    fn write(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazytools-test-settings-{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, body).unwrap();
        path
    }

    fn load(name: &str, body: &str) -> (Settings, Option<SettingsIssue>) {
        let path = write(name, body);
        let loaded = Settings::load_from(&path);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
        loaded
    }

    #[test]
    fn missing_file_is_not_an_error() {
        let (settings, issue) = Settings::load_from(Path::new("/does/not/exist/config.toml"));
        assert!(issue.is_none(), "a missing file is the normal path");
        assert_eq!(settings.restore, Restore::default());
    }

    #[test]
    fn restore_mode_is_read() {
        let (settings, issue) = load("restore-all", "[session]\nrestore = \"all\"\n");
        assert!(issue.is_none(), "{issue:?}");
        assert_eq!(settings.restore, Restore::All);
    }

    #[test]
    fn the_default_keeps_inputs_off_disk() {
        assert_eq!(Restore::default(), Restore::Options);
        assert!(!Restore::default().includes_inputs());
    }

    /// The whole point of `deny_unknown_fields`: a typo has to be visible.
    #[test]
    fn an_unknown_key_is_reported_and_names_itself() {
        let (settings, issue) = load("unknown", "[session]\nrestor = \"all\"\n");
        let msg = issue.expect("a misspelled key must be reported").message();
        assert!(msg.contains("restor"), "{msg}");
        assert_eq!(settings.restore, Restore::default());
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let (settings, issue) = load("malformed", "this is not toml === {{{");
        assert!(matches!(issue, Some(SettingsIssue::Malformed { .. })));
        assert_eq!(settings.restore, Restore::default());
    }

    #[test]
    fn theme_colors_are_applied() {
        let (settings, issue) = load(
            "theme-ok",
            "[theme]\nborder_focus = \"magenta\"\ntitle = \"#ff8800\"\ntext_dim = \"244\"\n",
        );
        assert!(issue.is_none(), "{issue:?}");
        assert_eq!(settings.theme.border_focus, Color::Magenta);
        assert_eq!(settings.theme.title, Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(settings.theme.text_dim, Color::Indexed(244));
        // Untouched entries keep the default.
        assert_eq!(settings.theme.error, Theme::default().error);
    }

    #[test]
    fn a_named_preset_becomes_the_theme() {
        let (settings, issue) = load("theme-name", "[theme]\nname = \"Dracula\"\n");
        assert!(issue.is_none(), "{issue:?}");
        assert_eq!(settings.theme_name.as_deref(), Some("dracula"));
        assert_eq!(settings.theme, themes::find("dracula").unwrap().theme);
    }

    /// The two halves of `[theme]` compose: the preset is the base, the
    /// individual colors are corrections on top of it. This also has to hold
    /// for a preset the *picker* moves to later, which is what `theme_for` is.
    #[test]
    fn a_color_entry_overrides_the_named_preset() {
        let (settings, issue) = load(
            "theme-name-and-color",
            "[theme]\nname = \"nord\"\nerror = \"magenta\"\n",
        );
        assert!(issue.is_none(), "{issue:?}");
        let nord = themes::find("nord").unwrap().theme;
        assert_eq!(settings.theme.text, nord.text, "the preset is the base");
        assert_eq!(settings.theme.error, Color::Magenta, "the entry wins");

        let previewed = settings.theme_for("gruvbox-dark");
        let gruvbox = themes::find("gruvbox-dark").unwrap().theme;
        assert_eq!(previewed.text, gruvbox.text);
        assert_eq!(
            previewed.error,
            Color::Magenta,
            "an override outlives the preset it was written against"
        );
    }

    #[test]
    fn an_unknown_theme_name_lists_the_ones_that_exist() {
        let (settings, issue) = load("theme-name-bad", "[theme]\nname = \"blurple\"\n");
        let msg = issue.expect("an unknown theme must be reported").message();
        assert!(msg.contains("dracula"), "{msg}");
        assert!(settings.theme_name.is_none());
        assert_eq!(settings.theme, Theme::default());
    }

    /// One bad entry must not cost the user the rest of the file — the reason
    /// `[theme]` is a map rather than a `deny_unknown_fields` struct.
    #[test]
    fn a_bad_color_is_skipped_and_the_rest_still_applies() {
        let (settings, issue) = load(
            "theme-bad",
            "[session]\nrestore = \"all\"\n\n[theme]\nborder_focus = \"blurple\"\nnot_a_slot = \"red\"\ntitle = \"green\"\n",
        );
        let entries = match issue {
            Some(SettingsIssue::Skipped { entries, .. }) => entries,
            other => panic!("expected Skipped, got {other:?}"),
        };
        assert_eq!(entries.len(), 2, "{entries:?}");
        assert_eq!(settings.theme.title, Color::Green, "valid colors apply");
        assert_eq!(
            settings.theme.border_focus,
            Theme::default().border_focus,
            "an unparsable color keeps the default"
        );
        assert_eq!(settings.restore, Restore::All, "other sections still apply");
    }

    #[test]
    fn an_unknown_restore_mode_is_skipped_without_touching_the_theme() {
        let (settings, issue) = load(
            "bad-mode",
            "[session]\nrestore = \"sometimes\"\n\n[theme]\ntitle = \"red\"\n",
        );
        let msg = issue.expect("an invalid mode must be reported").message();
        assert!(msg.contains("\"off\""), "{msg}");
        assert_eq!(settings.restore, Restore::default());
        assert_eq!(settings.theme.title, Color::Red);
    }
}
