//! Cross-session persistence, driven through `App` rather than through the
//! pure capture/restore functions — those have their own unit tests in
//! `session.rs`. What is checked here is that the wiring holds: the restored
//! tool is the one on screen, and a `Secret` typed into a live form does not
//! reach the file that gets written.

use std::path::PathBuf;

use lazytools::app::App;
use lazytools::session::Session;
use lazytools::settings::{Restore, Settings};
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        app.event(&key(KeyCode::Char(c))).expect("type char");
    }
}

fn screen(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("TestBackend must build");
    terminal
        .draw(|f| app.draw(f).expect("draw must not fail"))
        .expect("draw must run");
    terminal.backend().to_string()
}

fn settings(restore: Restore) -> Settings {
    Settings {
        restore,
        ..Settings::default()
    }
}

fn session(tool: &str, values: &[(&str, toml::Value)]) -> Session {
    Session {
        tool: Some(tool.to_string()),
        values: values
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
    }
}

/// Scratch directory private to each test, removed when done.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("lazytools-test-{name}"));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn the_saved_tool_is_the_one_that_opens() {
    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::Options),
        session(
            "convert.base64",
            &[("direction", toml::Value::String("decode".into()))],
        ),
    );
    let out = screen(&mut app, 100, 30);
    assert!(out.contains("Base64"), "the saved tool must open:\n{out}");
    assert!(out.contains("decode"), "and with its saved option:\n{out}");
}

/// The catalog moves; a session file does not. A tool that no longer exists
/// must leave the app where a fresh install would start, not on a blank form.
#[test]
fn a_tool_that_no_longer_exists_falls_back_to_the_default() {
    let mut restored = App::with_settings(
        Registry::new(),
        settings(Restore::Options),
        session("convert.removed-in-v9", &[]),
    );
    let mut fresh = App::new(Registry::new());
    assert_eq!(screen(&mut restored, 100, 30), screen(&mut fresh, 100, 30));
}

/// Everything else here is convenience. This one is the rule: a `Secret` typed
/// into the form must not appear in the bytes that get written, in the mode
/// that saves the most.
#[test]
fn a_secret_never_reaches_the_file() {
    let dir = TempDir::new("session-secret");
    let path = dir.join("session.toml");

    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::All),
        session("crypto.hmac", &[]),
    );
    // Into the form, then onto the key field (message → key).
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    type_str(&mut app, "the message");
    app.event(&key(KeyCode::Tab)).expect("next field");
    app.process_queue().expect("queue");
    type_str(&mut app, "sup3rs3cr3t");
    app.process_queue().expect("queue");

    let out = screen(&mut app, 100, 30);
    assert!(out.contains("HMAC"), "the HMAC tool must be open:\n{out}");

    app.session_snapshot().save_to(&path).expect("save session");
    let written = std::fs::read_to_string(&path).expect("session file");
    assert!(
        !written.contains("sup3rs3cr3t"),
        "a secret must never be written:\n{written}"
    );
    assert!(
        written.contains("the message"),
        "a non-secret input must be saved in Restore::All:\n{written}"
    );
}

/// The default mode keeps the tool and its options, and leaves the data alone.
#[test]
fn the_default_mode_saves_options_but_not_typed_input() {
    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::Options),
        session("convert.base64", &[]),
    );
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    type_str(&mut app, "not for the disk");
    app.process_queue().expect("queue");

    let snapshot = app.session_snapshot();
    assert_eq!(snapshot.tool.as_deref(), Some("convert.base64"));
    assert!(!snapshot.values.contains_key("text"));
    assert_eq!(
        snapshot
            .values
            .get("direction")
            .and_then(toml::Value::as_str),
        Some("encode")
    );
}

#[test]
fn restore_off_captures_nothing_at_all() {
    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::Off),
        session("convert.base64", &[]),
    );
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    type_str(&mut app, "nothing to see here");
    app.process_queue().expect("queue");

    let snapshot = app.session_snapshot();
    assert!(snapshot.tool.is_none(), "not even which tool was open");
    assert!(snapshot.values.is_empty());
}

/// With persistence off, the *stored* session is also not read back.
#[test]
fn restore_off_ignores_a_session_it_is_handed() {
    let mut off = App::with_settings(
        Registry::new(),
        settings(Restore::Off),
        session(
            "convert.base64",
            &[("direction", toml::Value::String("decode".into()))],
        ),
    );
    let out = screen(&mut off, 100, 30);
    assert!(
        !out.contains("decode"),
        "an off session must not restore values:\n{out}"
    );
}

/// A restored value has to reach the *run*, not just the widget: a Live tool
/// reopening on a saved option must show the result that option produces.
#[test]
fn a_restored_option_is_used_by_the_next_run() {
    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::All),
        session(
            "convert.base64",
            &[
                ("text", toml::Value::String("aGVsbG8=".into())),
                ("direction", toml::Value::String("decode".into())),
            ],
        ),
    );
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("hello"),
        "the restored input must be decoded with the restored option:\n{out}"
    );
}

/// A round trip through the real file, at the API the app uses on quit.
#[test]
fn what_is_written_is_what_comes_back() {
    let dir = TempDir::new("session-roundtrip");
    let path = dir.join("session.toml");

    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::Options),
        session(
            "convert.base64",
            &[("direction", toml::Value::String("decode".into()))],
        ),
    );
    app.process_queue().expect("queue");
    app.session_snapshot().save_to(&path).expect("save");

    let reloaded = Session::load_from(&path);
    assert_eq!(reloaded.tool.as_deref(), Some("convert.base64"));

    let mut next_run = App::with_settings(Registry::new(), settings(Restore::Options), reloaded);
    let out = screen(&mut next_run, 100, 30);
    assert!(out.contains("Base64"), "{out}");
    assert!(out.contains("decode"), "{out}");
}

/// The sidebar row currently drawn with the selection style, by its background
/// color — `to_string()` throws styles away, and the highlight *is* the thing
/// under test here.
fn highlighted_row(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("TestBackend must build");
    terminal
        .draw(|f| app.draw(f).expect("draw must not fail"))
        .expect("draw must run");
    let buf = terminal.backend().buffer().clone();
    for y in 0..buf.area.height {
        if buf[(1, y)].style().bg == Some(Color::Cyan) {
            return (1..23)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                .trim()
                .to_string();
        }
    }
    String::new()
}

/// Picking a tool from the palette must move the sidebar highlight with it.
/// It used to change the form only, leaving the list pointing somewhere else.
#[test]
fn selecting_from_the_palette_moves_the_sidebar_highlight() {
    let mut app = App::with_settings(Registry::new(), settings(Restore::Off), Session::default());
    assert_ne!(
        highlighted_row(&mut app),
        "ULID",
        "ULID must not be where the app starts, or this proves nothing"
    );

    app.event(&Event::Key(KeyEvent::new(
        KeyCode::Char('p'),
        KeyModifiers::CONTROL,
    )))
    .expect("open palette");
    app.process_queue().expect("queue");
    type_str(&mut app, "ulid");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("pick");
    app.process_queue().expect("queue");

    assert_eq!(highlighted_row(&mut app), "ULID");
}

/// Restoring a session moves it too — the list and the open form must agree
/// from the very first frame.
#[test]
fn a_restored_tool_is_the_highlighted_row() {
    let mut app = App::with_settings(
        Registry::new(),
        settings(Restore::Options),
        session("web.cron", &[]),
    );
    assert_eq!(highlighted_row(&mut app), "Cron Explainer");
}
