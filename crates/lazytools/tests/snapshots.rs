//! Snapshot tests via `TestBackend` — no real terminal needed.
//! First run generates `.snap.new`; review with `cargo insta review`.

use std::time::Duration;

use lazytools::app::App;
use lazytools_core::error::ToolError;
use lazytools_core::registry::{Registry, Tool};
use lazytools_core::spec::{Category, Field, ToolSpec};
use lazytools_core::value::{Inputs, Outputs};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
}

fn terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).expect("TestBackend must build")
}

fn draw(terminal: &mut Terminal<TestBackend>, app: &mut App) -> String {
    terminal
        .draw(|f| app.draw(f).expect("draw must not fail"))
        .expect("draw must run");
    terminal.backend().to_string()
}

fn render(width: u16, height: u16) -> String {
    let mut terminal = terminal(width, height);
    let mut app = App::new(Registry::new());
    draw(&mut terminal, &mut app)
}

#[test]
fn layout_default_100x30() {
    insta::assert_snapshot!(render(100, 30));
}

/// Responsive breakpoint: >=80 cols shows the full sidebar.
#[test]
fn layout_wide_120_cols() {
    insta::assert_snapshot!(render(120, 24));
}

/// 60..80 cols -> sidebar shrinks to icon-only.
#[test]
fn layout_narrow_70_cols() {
    insta::assert_snapshot!(render(70, 20));
}

/// <60 cols -> sidebar is hidden entirely, workspace takes up the whole width.
#[test]
fn layout_tiny_50_cols() {
    insta::assert_snapshot!(render(50, 16));
}

/// End-to-end in the TUI: type input -> debounce -> `registry.run()` -> digest appears.
/// The value must **match exactly** the CLI result for the same input.
#[test]
fn tool_form_shows_digest_matching_cli() {
    let mut terminal = terminal(90, 26);
    let mut app = App::new(Registry::new());

    // Tab: sidebar -> workspace, focus lands on the Input field.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");

    for c in "hello world".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type char");
    }
    app.process_queue().expect("queue");

    // Wait past the 80ms debounce threshold, then let `tick()` run the tool.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);

    assert!(
        screen.contains("5eb63bbbe01eeed093cb22bb8f5acdc3"),
        "the md5 digest of \"hello world\" must appear in the form:\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Safety threshold: input > 256KB makes `RunMode::Live` auto-downgrade to
/// run-on-demand with a badge, so pasting a large file doesn't hang the UI.
#[test]
fn large_input_downgrades_to_on_demand() {
    let mut terminal = terminal(90, 30);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");

    app.event(&Event::Paste("a".repeat(300 * 1024)))
        .expect("paste large chunk");
    app.process_queue().expect("queue");

    // Past the debounce threshold and it still doesn't auto-run — that's the intended behavior.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("large input"),
        "must show the badge prompting Enter:\n{screen}"
    );

    // Only pressing Enter runs it, and runs it correctly.
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.tick();
    let screen = draw(&mut terminal, &mut app);

    // md5 of 307200 'a' characters.
    let expected = {
        use md5::{Digest, Md5};
        hex::encode(Md5::digest("a".repeat(300 * 1024).as_bytes()))
    };
    assert!(
        screen.contains(&expected),
        "after pressing Enter the digest {expected} must be present:\n{screen}"
    );
}

/// Opening a `RunMode::OnDemand` tool must **not** run it. Bcrypt at the default cost 12
/// takes ~200ms on the UI thread, so auto-running it on open froze the TUI just to hash
/// an empty password. It stays idle until the run key is pressed.
#[test]
fn opening_an_on_demand_tool_does_not_run_it() {
    let mut terminal = terminal(90, 30);
    let mut app = App::new(Registry::new());

    app.event(&ctrl(KeyCode::Char('p'))).expect("ctrl+p");
    app.process_queue().expect("queue");
    for c in "bcrypt".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type");
    }
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("open tool");
    app.process_queue().expect("queue");

    // Well past the debounce deadline: a `Live` tool would have run by now.
    std::thread::sleep(Duration::from_millis(150));
    let started = std::time::Instant::now();
    app.tick();
    let elapsed = started.elapsed();

    let screen = draw(&mut terminal, &mut app);
    assert!(
        elapsed < Duration::from_millis(50),
        "tick() must not hash on open — it took {elapsed:?}:\n{screen}"
    );
    assert!(
        !screen.contains("$2"),
        "no hash may appear before the run key is pressed:\n{screen}"
    );
    assert!(
        screen.contains("press") && screen.contains("to run"),
        "the empty output must be explained by a run hint:\n{screen}"
    );

    // Focus the form, then Enter runs it for real.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("$2"),
        "pressing the run key must produce a bcrypt hash:\n{screen}"
    );
}

/// Palette opens with `Ctrl+P`; typing `md5` must bring Hash Text to the top.
#[test]
fn palette_matches_on_keywords() {
    let mut terminal = terminal(80, 24);
    let mut app = App::new(Registry::new());

    app.event(&ctrl(KeyCode::Char('p'))).expect("ctrl+p");
    app.process_queue().expect("queue");
    for c in "md5".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type");
    }
    app.process_queue().expect("queue");

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("Find tool") && screen.contains("Hash Text"),
        "palette must open and match `md5` -> Hash Text (matched via keywords):\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Help popup is built from `commands()` — must contain the exact key strings from `KeyConfig`.
#[test]
fn help_popup_is_generated_from_commands() {
    let mut terminal = terminal(80, 24);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Char('?'))).expect("?");
    app.process_queue().expect("queue");

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("Shortcuts") && screen.contains("^P") && screen.contains("quit"),
        "help must list the keys generated from commands():\n{screen}"
    );
    insta::assert_snapshot!(screen);
}

/// Test-only tool: covers **every** `FieldKind` variant to prove no variant is
/// missing a widget, and that `Secret` is actually masked.
struct AllKindsTool {
    spec: ToolSpec,
}

impl Default for AllKindsTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.allkinds", "All Kinds", Category::Text)
                .describe("covers every FieldKind")
                .input(Field::text("text").label("Text"))
                .option(Field::secret("key").label("Secret"))
                .option(Field::number("cost", 4, 15).default(12i64).label("Number"))
                .option(Field::toggle("flag").label("Toggle"))
                .option(
                    Field::select("mode", &["a", "b"])
                        .default("a")
                        .label("Select"),
                )
                .option(Field::filepath("path", false).label("FilePath"))
                .output(Field::text("result").label("Result")),
        }
    }
}

impl Tool for AllKindsTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        Ok(Outputs::one("result", i.text("text").to_uppercase()))
    }
}

#[test]
fn every_field_kind_renders_and_secret_is_masked() {
    let registry = Registry::from_tools(vec![Box::new(AllKindsTool::default())]);
    let mut terminal = terminal(80, 34);
    let mut app = App::new(registry);

    // Enter the form, type into the Text field, then Tab to the Secret field and type a key.
    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");
    for c in "hi".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type text");
    }
    app.event(&key(KeyCode::Tab)).expect("tab to secret");
    app.process_queue().expect("queue");
    for c in "s3cret".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type secret");
    }
    app.process_queue().expect("queue");
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);

    // Every FieldKind is present on screen.
    for label in ["Text", "Secret", "Number", "Toggle", "Select", "FilePath"] {
        assert!(
            screen.contains(label),
            "missing widget for {label}:\n{screen}"
        );
    }
    // Secret is masked; the real value never leaks into the buffer.
    assert!(
        !screen.contains("s3cret"),
        "the Secret value must NOT appear on screen:\n{screen}"
    );
    assert!(
        screen.contains("••••••"),
        "Secret must render masked:\n{screen}"
    );

    insta::assert_snapshot!(screen);
}

/// `y` is the copy key at the app level, but inside an input field it must be a
/// plain character. The output field is read-only so it doesn't consume the key,
/// and that's when `y` falls through to the app to copy — which is why "widget
/// first, app second" is the correct routing order.
#[test]
fn copy_key_still_types_inside_an_editable_field() {
    let mut terminal = terminal(90, 26);
    let mut app = App::new(Registry::new());

    app.event(&key(KeyCode::Tab)).expect("tab into form");
    app.process_queue().expect("queue");
    for c in "yes".chars() {
        app.event(&key(KeyCode::Char(c))).expect("type");
    }
    app.process_queue().expect("queue");
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("yes"),
        "`y` in the input field must be a character, not a copy command:\n{screen}"
    );
    // md5("yes")
    assert!(
        screen.contains("a6105c0a611b41b08f1209506350279e"),
        "the tool must run on exactly the typed string:\n{screen}"
    );
}
