//! Snapshot tests via `TestBackend` — no real terminal needed.
//! First run generates `.snap.new`; review with `cargo insta review`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use lazytools::app::App;
use lazytools_core::error::ToolError;
use lazytools_core::registry::{Registry, Tool};
use lazytools_core::spec::{Category, Field, RunMode, ToolSpec};
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

    // Drain the run request `set_tool` queued on open, while the input is still
    // empty. Without this the deadline survives the paste below and the very first
    // `tick()` hashes all 300KB — which is what this test is supposed to prove does
    // *not* happen. In the real event loop the 16ms poll clears it long before a
    // human can paste, so this is a test-harness artifact, not a user-facing path.
    app.tick();

    app.event(&key(KeyCode::Tab)).expect("tab");
    app.process_queue().expect("queue");

    app.event(&Event::Paste("a".repeat(300 * 1024)))
        .expect("paste large chunk");
    app.process_queue().expect("queue");

    // md5 of 307200 'a' characters.
    let expected = {
        use md5::{Digest, Md5};
        hex::encode(Md5::digest("a".repeat(300 * 1024).as_bytes()))
    };

    // Past the debounce threshold and it still doesn't auto-run — that's the intended behavior.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("large input"),
        "must show the badge prompting the run key:\n{screen}"
    );
    assert!(
        !screen.contains(&expected),
        "the downgrade must stop the 300KB hash from running on its own:\n{screen}"
    );

    // Focus is on the multiline Input, so `Enter` belongs to the field: it inserts a
    // line break and must NOT be hijacked into running the tool. This is the whole
    // point of the fix — the downgrade used to repurpose `Enter` underneath the very
    // field the user was still editing.
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        !screen.contains(&expected),
        "Enter in a multiline field must edit it, not run the tool:\n{screen}"
    );

    // The run key works from any field, and runs it correctly.
    app.event(&ctrl(KeyCode::Char('r'))).expect("run key");
    app.process_queue().expect("queue");
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("large input"),
        "the badge must still name the run key:\n{screen}"
    );

    // The newline inserted above is part of the input now, so hash that exact text.
    let with_newline = {
        use md5::{Digest, Md5};
        hex::encode(Md5::digest(
            format!("{}\n", "a".repeat(300 * 1024)).as_bytes(),
        ))
    };
    assert!(
        screen.contains(&with_newline),
        "the run key must hash the edited text (300KB + the newline):\n{screen}"
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

/// Test-only stand-in for the generators added in Phase 3: `RunMode::Generate`, no
/// inputs at all, one option. `run()` counts its calls, so the test asserts *behavior*
/// (ran on open, ran again on confirm) without depending on a random value.
struct CountingGeneratorTool {
    spec: ToolSpec,
    runs: &'static AtomicUsize,
}

impl CountingGeneratorTool {
    fn new(runs: &'static AtomicUsize) -> Self {
        Self {
            spec: ToolSpec::new("generate.counter", "Counter", Category::Generate)
                .describe("test-only generator")
                .option(Field::number("count", 1, 10).default(5i64).label("Count"))
                .output(Field::text("result").label("Result"))
                .mode(RunMode::Generate),
            runs,
        }
    }
}

impl Tool for CountingGeneratorTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn run(&self, _i: &Inputs) -> Result<Outputs, ToolError> {
        let n = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Outputs::one("result", format!("run #{n}")))
    }
}

/// The point of the third `RunMode`: `Live` gives no way to ask for a *different*
/// value, and `OnDemand` opens showing nothing.
#[test]
fn generate_mode_runs_on_open_and_reruns_on_confirm() {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let registry = Registry::from_tools(vec![Box::new(CountingGeneratorTool::new(&RUNS))]);
    let mut terminal = terminal(80, 20);
    let mut app = App::new(registry);

    // Opening alone must produce a result — this is what `OnDemand` would not do.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert_eq!(
        RUNS.load(Ordering::SeqCst),
        1,
        "must run on open:\n{screen}"
    );
    assert!(screen.contains("run #1"), "{screen}");

    // The confirm key asks for a fresh value.
    app.event(&key(KeyCode::Tab)).expect("tab into form");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Enter)).expect("enter");
    app.process_queue().expect("queue");
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    assert_eq!(
        RUNS.load(Ordering::SeqCst),
        2,
        "confirm must regenerate:\n{screen}"
    );
    assert!(screen.contains("run #2"), "{screen}");
}

#[test]
fn generate_mode_shows_regenerate_hint() {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let registry = Registry::from_tools(vec![Box::new(CountingGeneratorTool::new(&RUNS))]);
    let mut terminal = terminal(80, 20);
    let mut app = App::new(registry);

    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);

    assert!(
        screen.contains("regenerate"),
        "a Generate tool must offer `regenerate`, not `run`:\n{screen}"
    );
}

/// `set_primary_input` used to write into `widgets.first_mut()` unconditionally. With
/// no inputs the first widget is an *option*, so opening a file dumped its whole
/// contents into the `count` box.
#[test]
fn open_file_does_not_clobber_options_of_an_input_less_tool() {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let registry = Registry::from_tools(vec![Box::new(CountingGeneratorTool::new(&RUNS))]);
    let mut terminal = terminal(80, 20);
    let mut app = App::new(registry);

    let dir = std::env::temp_dir().join("lazytools-test-generate-openfile");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let file = dir.join("input.txt");
    std::fs::write(&file, "THIS MUST NOT LAND IN AN OPTION").expect("write sample file");

    app.open_file(&file);
    app.process_queue().expect("queue");
    std::thread::sleep(Duration::from_millis(150));
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        !screen.contains("THIS MUST NOT LAND"),
        "file content must not reach an option:\n{screen}"
    );
    assert!(
        screen.contains('5'),
        "the `count` option must still hold its default:\n{screen}"
    );
}

/// The command bar must not advertise a key that does nothing.
#[test]
fn open_file_is_not_advertised_for_an_input_less_tool() {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let registry = Registry::from_tools(vec![Box::new(CountingGeneratorTool::new(&RUNS))]);
    let mut terminal = terminal(100, 20);
    let mut app = App::new(registry);

    app.event(&key(KeyCode::Char('?'))).expect("?");
    app.process_queue().expect("queue");
    let screen = draw(&mut terminal, &mut app);

    assert!(
        !screen.contains("open file"),
        "a tool with no input must not advertise `open file`:\n{screen}"
    );
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

/// Test-only tool for the combination the UI layer used to forbid: a **multiline
/// input on an `OnDemand` tool**. Before the confirm key was handed to the focused
/// widget first, `ToolFormComponent` swallowed `Enter` as "run", so such a field
/// could never receive a line break and no tool could declare one.
///
/// `run()` reports the line count, so the test asserts the newlines actually landed
/// in the field rather than trusting the rendered box.
struct MultilineOnDemandTool {
    spec: ToolSpec,
}

impl Default for MultilineOnDemandTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("text.multi", "Multi", Category::Text)
                .describe("test-only multiline OnDemand tool")
                .input(Field::text("text").multiline().label("Input"))
                .output(Field::text("result").label("Result"))
                .mode(RunMode::OnDemand),
        }
    }
}

impl Tool for MultilineOnDemandTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }
    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        Ok(Outputs::one(
            "result",
            format!("lines={}", i.text("text").lines().count()),
        ))
    }
}

#[test]
fn multiline_field_on_an_on_demand_tool_gets_its_newlines() {
    let registry = Registry::from_tools(vec![Box::new(MultilineOnDemandTool::default())]);
    let mut terminal = terminal(80, 24);
    let mut app = App::new(registry);

    app.event(&key(KeyCode::Tab)).expect("tab into form");
    app.process_queue().expect("queue");

    // Three lines, typed the way a user would: text, Enter, text, Enter, text.
    for (i, word) in ["one", "two", "three"].iter().enumerate() {
        if i > 0 {
            app.event(&key(KeyCode::Enter)).expect("newline");
            app.process_queue().expect("queue");
        }
        for c in word.chars() {
            app.event(&key(KeyCode::Char(c))).expect("type");
        }
        app.process_queue().expect("queue");
    }

    // An OnDemand tool must still be idle — none of those Enters may have run it.
    std::thread::sleep(Duration::from_millis(150));
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        !screen.contains("lines="),
        "Enter must not have run an OnDemand tool:\n{screen}"
    );

    app.event(&ctrl(KeyCode::Char('r'))).expect("run key");
    app.process_queue().expect("queue");
    app.tick();
    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("lines=3"),
        "all three lines must have reached the tool:\n{screen}"
    );
}

/// The run key must work with focus on a read-only output too. `Enter` never could:
/// the old branch required an *editable* field, so a user looking at the result had
/// to Tab back into an input just to re-run.
#[test]
fn run_key_works_from_a_read_only_output() {
    let registry = Registry::from_tools(vec![Box::new(MultilineOnDemandTool::default())]);
    let mut terminal = terminal(80, 24);
    let mut app = App::new(registry);

    // Tab into the form, then Tab again to land on the read-only Result field.
    app.event(&key(KeyCode::Tab)).expect("tab into form");
    app.process_queue().expect("queue");
    app.event(&key(KeyCode::Tab)).expect("tab to output");
    app.process_queue().expect("queue");

    app.event(&ctrl(KeyCode::Char('r'))).expect("run key");
    app.process_queue().expect("queue");
    app.tick();

    let screen = draw(&mut terminal, &mut app);
    assert!(
        screen.contains("lines="),
        "the run key must fire from a read-only output:\n{screen}"
    );
}
