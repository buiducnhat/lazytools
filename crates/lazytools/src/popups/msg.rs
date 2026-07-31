use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::components::{CommandBlocking, CommandInfo, Component, DrawableComponent, EventState};
use crate::keys::{KeyConfig, key_match};
use crate::ui::{SharedTheme, centered_rect};

/// Message / error popup. Needed early because `catch_unwind` at the core layer
/// needs somewhere to display.
pub struct MsgPopup {
    title: &'static str,
    body: String,
    is_error: bool,
    visible: bool,
    theme: SharedTheme,
    key_config: KeyConfig,
}

impl MsgPopup {
    pub fn new(theme: SharedTheme, key_config: KeyConfig) -> Self {
        Self {
            title: "",
            body: String::new(),
            is_error: false,
            visible: false,
            theme,
            key_config,
        }
    }

    pub fn show_msg(&mut self, body: String) {
        self.title = " Message ";
        self.body = body;
        self.is_error = false;
        self.visible = true;
    }

    pub fn show_error(&mut self, body: String) {
        self.title = " Error ";
        self.body = body;
        self.is_error = true;
        self.visible = true;
    }
}

impl DrawableComponent for MsgPopup {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()> {
        if !self.visible {
            return Ok(());
        }
        let area = centered_rect(60, 30, rect);
        let style = if self.is_error {
            self.theme.error()
        } else {
            self.theme.text()
        };

        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(self.body.clone())
                .style(style)
                .wrap(Wrap { trim: false })
                .block(
                    Block::bordered()
                        .border_style(self.theme.block(true))
                        .title(self.title),
                ),
            area,
        );
        Ok(())
    }
}

impl Component for MsgPopup {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking {
        if self.visible || force_all {
            out.push(CommandInfo::new(
                self.key_config.hint(self.key_config.keys.exit_popup),
                "close",
                "Popup",
            ));
        }
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
            if key_match(k, keys.exit_popup) || key_match(k, keys.confirm) {
                self.hide();
            }
        }
        // While the popup is open it swallows every key — nothing leaks through
        // to the pane underneath.
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
}
