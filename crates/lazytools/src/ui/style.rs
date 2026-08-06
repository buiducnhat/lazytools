use std::rc::Rc;

use ratatui::style::{Color, Modifier, Style};

pub type SharedTheme = Rc<Theme>;

#[derive(Debug, Clone)]
pub struct Theme {
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
}
