//! File I/O in the TUI. These tests **write real files**, so they only touch
//! self-created temp directories — never files inside the repo.

use std::path::PathBuf;

use lazytools::app::App;
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
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

/// `Ctrl+S` opens the save popup; typing a new path then Enter writes right away (no prompt).
#[test]
fn saves_focused_output_to_a_new_file() {
    let dir = TempDir::new("save-new");
    let target = dir.join("out.txt");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    type_str(&mut app, "hello world");
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    // Tab to the Digest (output) field, then save.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");

    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    let written = std::fs::read_to_string(&target).expect("file must be created");
    assert_eq!(written, "5eb63bbbe01eeed093cb22bb8f5acdc3");
}

/// **Overwrite must always ask for confirmation** — the only non-undoable operation
/// in the whole app.
#[test]
fn overwrite_always_asks_first() {
    let dir = TempDir::new("save-overwrite");
    let target = dir.join("exists.txt");
    std::fs::write(&target, "OLD CONTENT").expect("create pre-existing file");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    type_str(&mut app, "hello world");
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");
    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    // The first Enter only opens the confirmation step — the file is untouched.
    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("overwrite"),
        "must ask for confirmation:\n{out}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "OLD CONTENT",
        "without confirmation it must NOT overwrite"
    );

    // Esc cancels -> content stays unchanged.
    app.event(&key(KeyCode::Esc)).expect("esc");
    app.process_queue().expect("queue");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "OLD CONTENT");

    // Enter -> back into confirmation, another Enter -> now it actually writes.
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("confirm");
    app.process_queue().expect("queue");
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "5eb63bbbe01eeed093cb22bb8f5acdc3"
    );
}

/// Missing parent directory -> reports an error, **without** creating the directory.
#[test]
fn missing_parent_directory_errors_without_creating_it() {
    let dir = TempDir::new("save-noparent");
    let missing = dir.join("does-not-exist");
    let target = missing.join("out.txt");

    let mut app = App::new(Registry::new());
    app.event(&key(KeyCode::Tab)).expect("enter form");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&ctrl(KeyCode::Char('s'))).expect("ctrl+s");
    app.process_queue().expect("queue");
    type_str(&mut app, target.to_str().unwrap());
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");

    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("does not exist"),
        "must report a clear error:\n{out}"
    );
    assert!(
        !missing.exists(),
        "must NOT auto-create the parent directory"
    );
    assert!(!target.exists());
}

/// `Ctrl+O` opens the picker; choosing a file loads its content into the primary
/// input and reruns the tool.
#[test]
fn opens_a_file_into_the_primary_input() {
    let dir = TempDir::new("open-file");
    let file = dir.join("input.txt");
    std::fs::write(&file, "hello world").expect("write sample file");

    let mut app = App::new(Registry::new());
    // Shortcut through the queue instead of simulating directory navigation —
    // directory browsing is the popup's job; here we check the load-into-form flow.
    app.event(&ctrl(KeyCode::Char('o'))).expect("ctrl+o");
    app.process_queue().expect("queue");
    let out = screen(&mut app, 100, 30);
    assert!(out.contains("Open file"), "picker must open:\n{out}");

    // Close the picker so the overlay doesn't hide the form when checking results.
    app.event(&key(KeyCode::Esc)).expect("esc");
    app.process_queue().expect("queue");

    app.open_file(&file);
    app.process_queue().expect("queue");
    std::thread::sleep(std::time::Duration::from_millis(150));
    app.tick();

    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("hello world"),
        "file content must land in the input:\n{out}"
    );
    assert!(
        out.contains("5eb63bbbe01eeed093cb22bb8f5acdc3"),
        "the tool must rerun on the new content:\n{out}"
    );
}

/// A file over the size limit is rejected with a message — no hang, no load.
#[test]
fn rejects_files_over_the_size_limit() {
    let dir = TempDir::new("open-toobig");
    let file = dir.join("big.txt");
    let limit = lazytools::popups::file_open::MAX_FILE_BYTES as usize;
    std::fs::write(&file, "a".repeat(limit + 1)).expect("write large file");

    let meta = std::fs::metadata(&file).unwrap();
    assert!(meta.len() > lazytools::popups::file_open::MAX_FILE_BYTES);

    let mut app = App::new(Registry::new());
    app.event(&ctrl(KeyCode::Char('o'))).expect("ctrl+o");
    app.process_queue().expect("queue");

    let rejected = lazytools::popups::file_open::check_openable(&file);
    assert!(rejected.is_err(), "an oversized file must be rejected");
    let msg = rejected.unwrap_err();
    assert!(
        msg.contains("limit"),
        "the message must state the limit: {msg}"
    );

    // And going through App, it also doesn't load into the form — just shows the error.
    app.open_file(&file);
    let out = screen(&mut app, 100, 30);
    assert!(
        out.contains("limit"),
        "must show the rejection reason:\n{out}"
    );
}
