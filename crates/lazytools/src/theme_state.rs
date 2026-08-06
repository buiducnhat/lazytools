//! `~/.local/state/lazytools/theme.toml` — the theme picked in the app.
//!
//! State, not config: [`paths`](crate::paths) keeps the two apart, and the rule
//! there is that the app never writes into a directory a person hand-edits. So
//! `Ctrl+T` cannot write `[theme] name` back into `config.toml`, and the pick
//! lands here instead.
//!
//! That leaves one question this module exists to answer: **which wins when
//! both name a theme?** Newest, and it can be worked out without a clock. The
//! file records the `config.toml` theme that was in force at the moment of the
//! pick, so:
//!
//! - the two agree → the pick was made against the config as it still is, and
//!   the pick wins,
//! - they disagree → `config.toml` has been edited since, and the file the
//!   user actually typed in wins.
//!
//! Deleting this file therefore always hands control back to `config.toml`,
//! and editing `config.toml` does too. Neither can be shadowed by a pick made
//! months ago.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ui::themes;

pub const FILE_NAME: &str = "theme.toml";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeState {
    /// The preset id chosen in the picker.
    pub name: String,
    /// What `config.toml`'s `[theme] name` was when that choice was made, if
    /// it named anything at all.
    #[serde(default)]
    pub from_config: Option<String>,
}

impl ThemeState {
    pub fn new(name: impl Into<String>, from_config: Option<String>) -> Self {
        Self {
            name: name.into(),
            from_config,
        }
    }

    pub fn path() -> Option<PathBuf> {
        crate::paths::state_file(FILE_NAME)
    }

    pub fn load() -> Option<Self> {
        Self::load_from(&Self::path()?)
    }

    /// Anything unreadable, unparseable, or naming a theme that no longer
    /// exists reads as "no pick" — `session.rs`'s rule, for the same reason:
    /// nobody wrote this file by hand, so there is nothing to report.
    pub fn load_from(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let state: Self = toml::from_str(&text).ok()?;
        themes::find(&state.name)?;
        Some(state)
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Self::path() {
            Some(path) => self.save_to(&path),
            // No HOME: nowhere to put it, and inventing a location is worse.
            None => Ok(()),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::paths::ensure_parent(path)?;
        std::fs::write(path, text)
    }
}

/// The preset to start in, given what `config.toml` names and what was last
/// picked. See the module docs for why a stale pick loses.
pub fn resolve(config_name: Option<&str>, picked: Option<&ThemeState>) -> String {
    match picked {
        Some(state) if state.from_config.as_deref() == config_name => state.name.clone(),
        _ => config_name.unwrap_or(themes::DEFAULT_ID).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lazytools-test-theme-{name}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn with_nothing_saved_the_config_decides() {
        assert_eq!(resolve(None, None), themes::DEFAULT_ID);
        assert_eq!(resolve(Some("nord"), None), "nord");
    }

    #[test]
    fn a_pick_made_against_the_current_config_wins() {
        let picked = ThemeState::new("dracula", None);
        assert_eq!(resolve(None, Some(&picked)), "dracula");

        let picked = ThemeState::new("dracula", Some("nord".into()));
        assert_eq!(resolve(Some("nord"), Some(&picked)), "dracula");
    }

    /// The case the `from_config` field exists for: someone edits
    /// `config.toml` after having picked a theme in the app. The file they
    /// just typed in has to win, or the setting looks broken.
    #[test]
    fn editing_the_config_afterwards_overrides_a_stale_pick() {
        let picked = ThemeState::new("dracula", Some("nord".into()));
        assert_eq!(resolve(Some("monokai"), Some(&picked)), "monokai");
        // Removing `name` from the config counts as an edit too.
        assert_eq!(resolve(None, Some(&picked)), themes::DEFAULT_ID);
    }

    #[test]
    fn a_pick_round_trips_through_the_file() {
        let path = dir("round-trip").join(FILE_NAME);
        let state = ThemeState::new("tokyo-night", Some("nord".into()));
        state.save_to(&path).unwrap();
        assert_eq!(ThemeState::load_from(&path), Some(state));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn saving_creates_the_state_directory() {
        let root = dir("mkdir");
        let path = root.join("nested").join(FILE_NAME);
        ThemeState::new("nord", None).save_to(&path).unwrap();
        assert!(path.is_file());
        std::fs::remove_dir_all(&root).ok();
    }

    /// A theme that has been renamed or removed since the pick was made must
    /// not leave the app starting on a theme it can no longer resolve.
    #[test]
    fn a_corrupt_or_stale_file_reads_as_no_pick() {
        let root = dir("stale");
        let path = root.join(FILE_NAME);

        std::fs::write(&path, "not { toml ===").unwrap();
        assert_eq!(ThemeState::load_from(&path), None);

        std::fs::write(&path, "name = \"a-theme-that-was-removed\"\n").unwrap();
        assert_eq!(ThemeState::load_from(&path), None);

        assert_eq!(ThemeState::load_from(&root.join("absent.toml")), None);
        std::fs::remove_dir_all(&root).ok();
    }
}
