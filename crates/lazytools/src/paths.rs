//! Where the app's files live.
//!
//! Two directories, kept apart on purpose:
//!
//! - **config** — files a *person* writes (`keys.toml`, `config.toml`). Never
//!   written by the app.
//! - **state** — files the *app* writes (`session.toml`). Deleting that whole
//!   directory must cost the user nothing but their last-open tool.
//!
//! No `dirs` crate: two paths and an environment variable each is less code
//! than the dependency, and it keeps the tree honest about what it pulls in.

use std::path::{Path, PathBuf};

/// `$HOME`, or `%USERPROFILE%` on Windows. `None` when neither is set — every
/// caller then falls back to defaults rather than guessing at a location.
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

/// Value of `var` when it holds an absolute path — the XDG spec says a relative
/// one is to be ignored, not resolved.
fn xdg_dir(var: &str) -> Option<PathBuf> {
    absolute_only(std::env::var_os(var))
}

fn absolute_only(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// Candidate config directories, most preferred first: `$XDG_CONFIG_HOME` when
/// set, then `~/.config`.
///
/// Both, not one: `~/.config/lazytools/keys.toml` is where every existing
/// install already keeps its bindings, and honoring `XDG_CONFIG_HOME` must not
/// mean silently ignoring a file that worked yesterday.
fn config_dirs() -> Vec<PathBuf> {
    config_dirs_from(xdg_dir("XDG_CONFIG_HOME"), home())
}

/// The environment is a parameter here rather than a read, so the resolution
/// order is testable: `std::env::set_var` is process-global (and `unsafe` in
/// edition 2024), which makes env-mutating tests race each other.
fn config_dirs_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(xdg) = xdg {
        dirs.push(xdg);
    }
    if let Some(home) = home {
        let dot_config = home.join(".config");
        if !dirs.contains(&dot_config) {
            dirs.push(dot_config);
        }
    }
    dirs.into_iter().map(|d| d.join("lazytools")).collect()
}

/// Path to a config file: the first candidate that exists, or the preferred one
/// when none does — so an error message names the place the user should create.
pub fn config_file(name: &str) -> Option<PathBuf> {
    pick_config_file(&config_dirs(), name)
}

fn pick_config_file(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = dirs.iter().map(|d| d.join(name)).collect();
    candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

/// `$XDG_STATE_HOME/lazytools`, else `~/.local/state/lazytools`.
///
/// State rather than config because nobody hand-edits a session file, and a
/// config directory under version control should not start carrying one
/// machine's last-open tool.
pub fn state_file(name: &str) -> Option<PathBuf> {
    Some(state_dir_from(xdg_dir("XDG_STATE_HOME"), home())?.join(name))
}

fn state_dir_from(xdg: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = xdg.or_else(|| home.map(|h| h.join(".local").join("state")))?;
    Some(base.join("lazytools"))
}

/// Creates the parent directory of `path` if it isn't there yet.
pub fn ensure_parent(path: &Path) -> std::io::Result<()> {
    match path.parent() {
        Some(dir) => std::fs::create_dir_all(dir),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn xdg_comes_first_but_dot_config_is_still_searched() {
        assert_eq!(
            config_dirs_from(Some(p("/xdg")), Some(p("/home/u"))),
            vec![p("/xdg/lazytools"), p("/home/u/.config/lazytools")]
        );
    }

    /// An install that never set `XDG_CONFIG_HOME` must keep working, and one
    /// that sets it to the same place must not get the directory twice.
    #[test]
    fn without_xdg_it_is_just_dot_config() {
        assert_eq!(
            config_dirs_from(None, Some(p("/home/u"))),
            vec![p("/home/u/.config/lazytools")]
        );
        assert_eq!(
            config_dirs_from(Some(p("/home/u/.config")), Some(p("/home/u"))),
            vec![p("/home/u/.config/lazytools")]
        );
    }

    #[test]
    fn with_no_home_at_all_there_is_nowhere_to_look() {
        assert!(config_dirs_from(None, None).is_empty());
        assert_eq!(pick_config_file(&[], "config.toml"), None);
        assert_eq!(state_dir_from(None, None), None);
    }

    #[test]
    fn state_defaults_under_local_state() {
        assert_eq!(
            state_dir_from(None, Some(p("/home/u"))),
            Some(p("/home/u/.local/state/lazytools"))
        );
        assert_eq!(
            state_dir_from(Some(p("/xdg-state")), Some(p("/home/u"))),
            Some(p("/xdg-state/lazytools"))
        );
    }

    /// A relative `XDG_*` is ignored by the spec, not resolved against the cwd.
    #[test]
    fn a_relative_xdg_value_is_ignored() {
        assert_eq!(absolute_only(Some("/xdg".into())), Some(p("/xdg")));
        assert_eq!(absolute_only(Some("relative/dir".into())), None);
        assert_eq!(absolute_only(Some("".into())), None);
        assert_eq!(absolute_only(None), None);
    }

    /// The whole reason both directories are searched: a file that exists in
    /// `~/.config` must win over a preferred-but-absent XDG location.
    #[test]
    fn an_existing_file_wins_over_the_preferred_empty_directory() {
        let root = std::env::temp_dir().join("lazytools-test-paths");
        std::fs::remove_dir_all(&root).ok();
        let xdg = root.join("xdg").join("lazytools");
        let dot = root.join("home").join(".config").join("lazytools");
        std::fs::create_dir_all(&dot).unwrap();
        std::fs::write(dot.join("config.toml"), "").unwrap();

        let dirs = vec![xdg.clone(), dot.clone()];
        assert_eq!(
            pick_config_file(&dirs, "config.toml"),
            Some(dot.join("config.toml")),
            "the file that exists is the one to read"
        );
        assert_eq!(
            pick_config_file(&dirs, "keys.toml"),
            Some(xdg.join("keys.toml")),
            "with none present, name the preferred location"
        );
        std::fs::remove_dir_all(&root).ok();
    }
}
