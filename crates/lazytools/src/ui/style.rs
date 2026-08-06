use std::cell::Cell;
use std::rc::Rc;

use ratatui::style::{Color, Modifier, Style};

/// What every component holds. The handle rather than the `Theme` itself
/// because the theme picker changes colors *while the app is running*: a
/// component that had been given a copy would keep drawing the old one.
pub type SharedTheme = Rc<ThemeHandle>;

/// The nine colors the whole interface is drawn from.
///
/// `Copy` on purpose — it is nine `Color`s, and being able to hand one out by
/// value is what lets `ThemeHandle` use a `Cell` instead of a `RefCell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub background: Color,
    pub border: Color,
    pub border_focus: Color,
    pub text: Color,
    pub text_dim: Color,
    pub error: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
    pub title: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // `Reset` means the terminal's own background — the app paints no
            // color of its own unless a theme asks for one.
            background: Color::Reset,
            border: Color::DarkGray,
            border_focus: Color::Cyan,
            text: Color::Reset,
            text_dim: Color::DarkGray,
            error: Color::Red,
            selection_fg: Color::Black,
            selection_bg: Color::Cyan,
            title: Color::Cyan,
        }
    }
}

/// Every slot's name, in the order a person would read them. Used by the
/// settings loader's error messages and by the docs; the picker shows colors,
/// not names.
pub const SLOTS: &[&str] = &[
    "background",
    "border",
    "border_focus",
    "text",
    "text_dim",
    "error",
    "selection_fg",
    "selection_bg",
    "title",
];

/// Parses a color written in `config.toml`.
///
/// Three notations, in the order people reach for them:
///
/// - a name — `cyan`, `dark-gray`, `light-blue`, or `reset` for the terminal's
///   own foreground/background,
/// - `#rrggbb` — exact, but only on a truecolor terminal,
/// - `0`–`255` — an index into the 256-color palette, which is what the named
///   colors above are anyway.
///
/// Named colors are the ones that follow the user's terminal theme, so they
/// stay the default: a hard-coded `#1e1e2e` looks wrong the moment someone
/// switches to a light background.
pub fn parse_color(spec: &str) -> Option<Color> {
    let s = spec.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let n = u32::from_str_radix(hex, 16).ok()?;
        return Some(Color::Rgb(
            ((n >> 16) & 0xff) as u8,
            ((n >> 8) & 0xff) as u8,
            (n & 0xff) as u8,
        ));
    }
    if let Ok(index) = s.parse::<u8>() {
        return Some(Color::Indexed(index));
    }
    // `-` and `_` both accepted: `dark-gray`, `dark_gray`, `darkgray`.
    let name = s.replace(['-', '_'], "");
    let color = match name.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => return None,
    };
    Some(color)
}

impl Theme {
    /// Overrides one color by name. `false` if the theme has no such color —
    /// the settings loader turns that into a message rather than a no-op.
    pub fn set_color(&mut self, name: &str, color: Color) -> bool {
        match name {
            "background" => self.background = color,
            "border" => self.border = color,
            "border_focus" => self.border_focus = color,
            "text" => self.text = color,
            "text_dim" => self.text_dim = color,
            "error" => self.error = color,
            "selection_fg" => self.selection_fg = color,
            "selection_bg" => self.selection_bg = color,
            "title" => self.title = color,
            _ => return false,
        }
        true
    }

    /// The surface everything else is drawn on. Painted over the whole frame
    /// before anything else, and again under each popup — a popup clears the
    /// cells beneath it, which would otherwise punch a hole in the background.
    pub fn base(&self) -> Style {
        Style::default().fg(self.text).bg(self.background)
    }

    pub fn block(&self, focused: bool) -> Style {
        Style::default().fg(if focused {
            self.border_focus
        } else {
            self.border
        })
    }

    pub fn title(&self, focused: bool) -> Style {
        let style = Style::default().fg(if focused { self.title } else { self.text_dim });
        if focused {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    pub fn text(&self) -> Style {
        Style::default().fg(self.text)
    }

    pub fn dim(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    pub fn error(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn selection(&self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    /// Group heading in the sidebar.
    pub fn group(&self) -> Style {
        Style::default().fg(self.title).add_modifier(Modifier::BOLD)
    }
}

/// The theme every component draws through.
///
/// A `Cell`, not a `RefCell`: `Theme` is `Copy`, so reading one out during a
/// draw needs no borrow and cannot panic on a re-entrant read. Swapping the
/// contents is how the picker previews a theme — every component already holds
/// this same handle, so one write re-themes the whole screen on the next frame.
#[derive(Debug, Default)]
pub struct ThemeHandle(Cell<Theme>);

impl ThemeHandle {
    pub fn new(theme: Theme) -> Self {
        Self(Cell::new(theme))
    }

    pub fn get(&self) -> Theme {
        self.0.get()
    }

    pub fn set(&self, theme: Theme) {
        self.0.set(theme);
    }

    pub fn base(&self) -> Style {
        self.get().base()
    }

    pub fn block(&self, focused: bool) -> Style {
        self.get().block(focused)
    }

    pub fn title(&self, focused: bool) -> Style {
        self.get().title(focused)
    }

    pub fn text(&self) -> Style {
        self.get().text()
    }

    pub fn dim(&self) -> Style {
        self.get().dim()
    }

    pub fn error(&self) -> Style {
        self.get().error()
    }

    pub fn selection(&self) -> Style {
        self.get().selection()
    }

    pub fn group(&self) -> Style {
        self.get().group()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_notations() {
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(0xff, 0x88, 0x00)));
        assert_eq!(parse_color("244"), Some(Color::Indexed(244)));
        assert_eq!(parse_color("reset"), Some(Color::Reset));
    }

    #[test]
    fn names_are_forgiving_about_case_and_separators() {
        for spec in ["dark-gray", "dark_gray", "DarkGray", " darkgrey "] {
            assert_eq!(parse_color(spec), Some(Color::DarkGray), "{spec}");
        }
    }

    #[test]
    fn rejects_what_it_cannot_render() {
        assert_eq!(parse_color("blurple"), None);
        assert_eq!(parse_color(""), None);
        // Three-digit hex is not accepted — `#f80` would have to be guessed at.
        assert_eq!(parse_color("#f80"), None);
        assert_eq!(parse_color("#gggggg"), None);
        // Out of the 256-color range, rather than silently wrapping.
        assert_eq!(parse_color("256"), None);
    }

    #[test]
    fn set_color_reports_a_slot_that_does_not_exist() {
        let mut theme = Theme::default();
        assert!(theme.set_color("border_focus", Color::Red));
        assert_eq!(theme.border_focus, Color::Red);
        assert!(!theme.set_color("border_focused", Color::Red));
    }

    /// `SLOTS` is what the error messages and the docs list. A slot missing
    /// from it is a slot nobody can discover.
    #[test]
    fn every_named_slot_is_settable() {
        let mut theme = Theme::default();
        for slot in SLOTS {
            assert!(theme.set_color(slot, Color::Red), "{slot}");
        }
        assert_eq!(
            theme,
            Theme {
                background: Color::Red,
                border: Color::Red,
                border_focus: Color::Red,
                text: Color::Red,
                text_dim: Color::Red,
                error: Color::Red,
                selection_fg: Color::Red,
                selection_bg: Color::Red,
                title: Color::Red,
            },
            "SLOTS must name every field of the theme"
        );
    }

    /// The handle is what makes live preview possible: components hold a clone
    /// of the `Rc`, so a write here is visible to all of them.
    #[test]
    fn a_write_through_one_handle_is_seen_by_every_holder() {
        let handle: SharedTheme = Rc::new(ThemeHandle::new(Theme::default()));
        let held_elsewhere = handle.clone();
        handle.set(Theme {
            title: Color::Magenta,
            ..Theme::default()
        });
        assert_eq!(held_elsewhere.get().title, Color::Magenta);
    }
}
