pub mod style;
pub mod themes;

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub use style::{SLOTS, SharedTheme, Theme, ThemeHandle, parse_color};

/// `true` when `(col, row)` is strictly inside `r`. Used by the mouse-routing layer:
/// every component compares the click position to its last drawn `Rect` and only
/// claims the click when the comparison is true. Strict inequality on the right/bottom
/// edge matches ratatui's own `Rect::contains` and avoids the "click on the seam
/// between two adjacent rects claims both" ambiguity.
pub fn inside(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

/// A centered rectangular area, used for popups.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
