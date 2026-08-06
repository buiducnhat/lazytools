//! The `[theme]` section, checked where it matters: the cells that end up on
//! screen. `parse_color` has its own unit tests in `ui/style.rs`; what those
//! can't show is that a parsed color is actually the one drawn.

use lazytools::app::App;
use lazytools::session::Session;
use lazytools::settings::Settings;
use lazytools::ui::Theme;
use lazytools_core::registry::Registry;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;

fn render(theme: Theme) -> Buffer {
    let settings = Settings {
        theme,
        ..Settings::default()
    };
    let mut app = App::with_settings(Registry::new(), settings, Session::default());
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("TestBackend must build");
    terminal
        .draw(|f| app.draw(f).expect("draw must not fail"))
        .expect("draw must run");
    terminal.backend().buffer().clone()
}

fn foregrounds(buf: &Buffer) -> Vec<Color> {
    (0..buf.area.width)
        .flat_map(|x| (0..buf.area.height).map(move |y| (x, y)))
        .filter_map(|(x, y)| buf[(x, y)].style().fg)
        .collect()
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
    let found = (0..themed.area.width)
        .flat_map(|x| (0..themed.area.height).map(move |y| (x, y)))
        .any(|(x, y)| themed[(x, y)].style().bg == Some(Color::Indexed(129)));
    assert!(found, "the selected row must use the configured background");
}
