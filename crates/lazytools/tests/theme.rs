//! The `[theme]` section and the `Ctrl+T` picker, checked where it matters:
//! the cells that end up on screen. `parse_color` and the preset table have
//! their own unit tests in `ui/`; what those can't show is that a chosen color
//! is actually the one drawn.

use lazytools::app::App;
use lazytools::session::Session;
use lazytools::settings::Settings;
use lazytools::theme_state::{self, ThemeState};
use lazytools::ui::{Theme, themes};
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
}

fn app_with(settings: Settings) -> App {
    App::with_settings(Registry::new(), settings, Session::default())
}

fn buffer(app: &mut App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("TestBackend must build");
    terminal
        .draw(|f| app.draw(f).expect("draw must not fail"))
        .expect("draw must run");
    terminal.backend().buffer().clone()
}

fn render(theme: Theme) -> Buffer {
    buffer(&mut app_with(Settings {
        theme,
        ..Settings::default()
    }))
}

fn cells(buf: &Buffer) -> impl Iterator<Item = &ratatui::buffer::Cell> {
    (0..buf.area.width)
        .flat_map(|x| (0..buf.area.height).map(move |y| (x, y)))
        .map(|(x, y)| &buf[(x, y)])
}

fn foregrounds(buf: &Buffer) -> Vec<Color> {
    cells(buf).filter_map(|c| c.style().fg).collect()
}

fn backgrounds(buf: &Buffer) -> Vec<Color> {
    cells(buf).filter_map(|c| c.style().bg).collect()
}

/// Drives a key and drains the queue behind it — every theme change travels
/// through the queue, so a test that skips this sees the previous frame.
fn press(app: &mut App, ev: &Event) {
    app.event(ev).expect("event");
    app.process_queue().expect("queue");
}

/// The sidebar is focused at startup, so its border carries `border_focus`.
#[test]
fn a_configured_color_reaches_the_screen() {
    let default = render(Theme::default());
    assert!(
        foregrounds(&default).contains(&Color::Cyan),
        "the default focus border is cyan — the premise of this test"
    );

    let themed = render(Theme {
        border_focus: Color::Rgb(0xff, 0x00, 0x88),
        ..Theme::default()
    });
    assert!(
        foregrounds(&themed).contains(&Color::Rgb(0xff, 0x00, 0x88)),
        "the configured focus border must be the one drawn"
    );
}

/// A theme is passed to every component, not just the ones `App` draws itself.
#[test]
fn the_selection_color_reaches_the_sidebar_list() {
    let themed = render(Theme {
        selection_bg: Color::Indexed(129),
        ..Theme::default()
    });
    assert!(
        backgrounds(&themed).contains(&Color::Indexed(129)),
        "the selected row must use the configured background"
    );
}

/// The default theme paints no background of its own, so a terminal with a
/// picture behind it keeps showing through. Only a preset that names one does.
#[test]
fn the_default_theme_leaves_the_terminals_own_background_alone() {
    let default = render(Theme::default());
    assert!(
        !backgrounds(&default)
            .iter()
            .any(|c| matches!(c, Color::Rgb(..))),
        "nothing may paint an absolute background under the default theme"
    );
}

#[test]
fn the_picker_opens_on_the_theme_in_use_and_lists_the_rest() {
    let mut app = app_with(Settings::default());
    press(&mut app, &ctrl(KeyCode::Char('t')));

    let screen = buffer(&mut app)
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(screen.contains("Theme"), "the picker must be titled");
    for name in ["Terminal", "Dracula", "Nord", "Solarized Light"] {
        assert!(screen.contains(name), "{name} must be listed:\n{screen}");
    }
}

/// The point of previewing rather than applying: moving the cursor re-themes
/// the app *behind* the popup, so the choice is made by looking at the real
/// thing. Cancelling must then leave nothing behind.
#[test]
fn moving_the_cursor_previews_and_esc_puts_the_old_theme_back() {
    let dracula = themes::find("dracula").expect("shipped preset").theme;
    let mut app = app_with(Settings::default());

    press(&mut app, &ctrl(KeyCode::Char('t')));
    press(&mut app, &key(KeyCode::Down));
    assert!(
        backgrounds(&buffer(&mut app)).contains(&dracula.background),
        "the previewed theme must already be on screen"
    );

    press(&mut app, &key(KeyCode::Esc));
    let after = buffer(&mut app);
    assert!(
        !backgrounds(&after).contains(&dracula.background),
        "a cancelled picker must leave no trace of what was tried"
    );
    assert!(
        app.theme_snapshot().is_none(),
        "and nothing to write down either"
    );
    assert!(
        foregrounds(&after).contains(&Color::Cyan),
        "the theme in force before the picker opened must be back"
    );
}

#[test]
fn confirming_keeps_the_theme_and_records_the_choice() {
    let dracula = themes::find("dracula").expect("shipped preset").theme;
    let mut app = app_with(Settings::default());

    press(&mut app, &ctrl(KeyCode::Char('t')));
    press(&mut app, &key(KeyCode::Down));
    press(&mut app, &key(KeyCode::Enter));

    let after = buffer(&mut app);
    assert!(
        backgrounds(&after).contains(&dracula.background),
        "the confirmed theme must stay on screen after the popup closes"
    );

    let snapshot = app
        .theme_snapshot()
        .expect("a confirmed pick must be recorded");
    assert_eq!(snapshot.name, "dracula");
    assert_eq!(
        snapshot.from_config, None,
        "nothing in config.toml named a theme, and that has to be recorded too"
    );
    assert_eq!(
        theme_state::resolve(None, Some(&snapshot)),
        "dracula",
        "the next run must open in the theme that was picked"
    );
}

/// The picker changes the *base*; the colors someone wrote in `config.toml`
/// are corrections that outlive it. Losing them on a theme switch would make
/// the two features exclusive.
#[test]
fn a_configured_color_survives_switching_themes() {
    let mut settings = Settings {
        theme_overrides: vec![("title".to_string(), Color::Indexed(129))],
        ..Settings::default()
    };
    settings.theme = settings.theme_for(themes::DEFAULT_ID);

    let mut app = app_with(settings);
    assert!(foregrounds(&buffer(&mut app)).contains(&Color::Indexed(129)));

    press(&mut app, &ctrl(KeyCode::Char('t')));
    press(&mut app, &key(KeyCode::Down));
    press(&mut app, &key(KeyCode::Enter));

    let after = buffer(&mut app);
    let dracula = themes::find("dracula").expect("shipped preset").theme;
    assert!(
        backgrounds(&after).contains(&dracula.background),
        "the preset must have been applied"
    );
    assert!(
        foregrounds(&after).contains(&Color::Indexed(129)),
        "the configured color must still win over the new preset"
    );
    assert!(
        !foregrounds(&after).contains(&dracula.title),
        "the preset's own title color must not come back"
    );
}

/// The pick is written to the state directory, never into the config file the
/// user hand-edits — and it has to survive the round trip.
#[test]
fn the_choice_round_trips_through_the_state_file() {
    let dir = std::env::temp_dir().join("lazytools-test-theme-app");
    std::fs::remove_dir_all(&dir).ok();
    let path = dir.join(lazytools::theme_state::FILE_NAME);

    let mut app = app_with(Settings::default());
    press(&mut app, &ctrl(KeyCode::Char('t')));
    press(&mut app, &key(KeyCode::Down));
    press(&mut app, &key(KeyCode::Enter));
    app.theme_snapshot()
        .expect("a confirmed pick must be recorded")
        .save_to(&path)
        .expect("save the pick");

    let loaded = ThemeState::load_from(&path).expect("the file must read back");
    assert_eq!(loaded.name, "dracula");
    std::fs::remove_dir_all(&dir).ok();
}

/// A run that never touches the picker must not create a `theme.toml`. If it
/// did, that file would then start shadowing later edits to `config.toml` for
/// someone who never chose a theme in the first place.
#[test]
fn a_run_that_never_opened_the_picker_has_nothing_to_save() {
    let app = app_with(Settings::default());
    assert!(app.theme_snapshot().is_none());
    // Safe against the real state directory precisely because of the above.
    app.persist_theme().expect("writing nothing cannot fail");
}
