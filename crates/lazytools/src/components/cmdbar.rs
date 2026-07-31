use anyhow::Result;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{CommandInfo, DrawableComponent};
use crate::ui::SharedTheme;

/// Thanh hint một dòng. Nội dung đến từ `commands()` của các component đang hiển
/// thị, nên không bao giờ lỗi thời so với code xử lý phím.
pub struct CommandBar {
    cmds: Vec<CommandInfo>,
    theme: SharedTheme,
}

impl CommandBar {
    pub fn new(theme: SharedTheme) -> Self {
        Self {
            cmds: Vec::new(),
            theme,
        }
    }

    pub fn set_cmds(&mut self, mut cmds: Vec<CommandInfo>) {
        cmds.sort_by_key(|c| c.order);
        self.cmds = cmds;
    }
}

impl DrawableComponent for CommandBar {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        let mut spans = Vec::new();
        for cmd in self.cmds.iter().filter(|c| c.enabled) {
            spans.push(Span::styled(
                format!("[{}]", cmd.key),
                self.theme.title(true),
            ));
            spans.push(Span::styled(format!(" {} ", cmd.label), self.theme.dim()));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
        Ok(())
    }
}
