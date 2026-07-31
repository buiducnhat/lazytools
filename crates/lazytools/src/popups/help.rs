//! Help popup. Its content is **generated from the `commands()`** of the
//! currently visible components, not a hand-written list — same reason as the
//! cmdbar: help never drifts from the real key bindings.

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::components::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match};
use crate::ui::{SharedTheme, centered_rect};

pub struct HelpPopup {
    cmds: Vec<CommandInfo>,
    visible: bool,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl HelpPopup {
    pub fn new(theme: SharedTheme, key_config: KeyConfig) -> Self {
        Self {
            cmds: Vec::new(),
            visible: false,
            theme,
            key_config,
        }
    }

    /// `App` loads in the full command list (`force_all`) right before opening.
    pub fn set_cmds(&mut self, mut cmds: Vec<CommandInfo>) {
        cmds.sort_by(|a, b| a.group.cmp(b.group).then(a.order.cmp(&b.order)));
        cmds.dedup_by(|a, b| a.group == b.group && a.key == b.key && a.label == b.label);
        self.cmds = cmds;
    }
}

impl DrawableComponent for HelpPopup {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(60, 70, rect);
        f.render_widget(Clear, area);

        let mut lines = Vec::new();
        let mut current_group = "";
        for cmd in &self.cmds {
            if cmd.group != current_group {
                if !current_group.is_empty() {
                    lines.push(Line::from(""));
                }
                current_group = cmd.group;
                lines.push(Line::from(Span::styled(
                    current_group.to_string(),
                    self.theme.group(),
                )));
            }
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<10}", cmd.key), self.theme.title(true)),
                Span::styled(cmd.label.to_string(), self.theme.text()),
            ]));
        }

        f.render_widget(
            Paragraph::new(lines).block(
                Block::bordered()
                    .border_style(self.theme.block(true))
                    .title_style(self.theme.title(true))
                    .title(" Shortcuts "),
            ),
            area,
        );
        Ok(())
    }
}

impl Component for HelpPopup {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        // The command that *opens* help is an app-level affordance; only the close
        // command lives here.
        if self.visible {
            out.push(
                CommandInfo::new(
                    self.key_config.hint(self.key_config.keys.exit_popup),
                    "close",
                    "Popup",
                )
                .order(1),
            );
        }
        let _ = force_all;
        if self.visible {
            CommandBlocking::Blocking
        } else {
            CommandBlocking::PassingOn
        }
    }

    fn event(&mut self, ev: &Event) -> Result<EventState> {
        if !self.visible {
            return Ok(EventState::NotConsumed);
        }
        if let Event::Key(k) = ev {
            let keys = &self.key_config.keys;
            if key_match(k, keys.exit_popup) || key_match(k, keys.quit) {
                self.hide();
            }
        }
        Ok(EventState::Consumed)
    }

    fn focused(&self) -> bool {
        self.visible
    }

    fn set_focused(&mut self, _focused: bool) {}

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn show(&mut self) -> Result<()> {
        self.visible = true;
        Ok(())
    }
}
