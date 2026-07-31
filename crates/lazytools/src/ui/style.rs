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

impl Theme {
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
