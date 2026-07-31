use anyhow::Result;
use lazytools_core::spec::Field;
use lazytools_core::value::Value;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};

use super::FieldWidget;
use crate::components::EventState;
use crate::keys::{KeyConfig, key_match};
use crate::ui::SharedTheme;

pub struct SelectWidget {
    key: &'static str,
    label: &'static str,
    options: &'static [&'static str],
    index: usize,
    error: Option<String>,
    theme: SharedTheme,
}

impl SelectWidget {
    pub fn new(field: &Field, options: &'static [&'static str], theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            options,
            index: 0,
            error: None,
            theme,
        }
    }

    fn current(&self) -> &'static str {
        self.options.get(self.index).copied().unwrap_or_default()
    }
}

impl FieldWidget for SelectWidget {
    fn key(&self) -> &'static str {
        self.key
    }

    fn value(&self) -> Value {
        Value::Choice(self.current().to_string())
    }

    fn set_value(&mut self, v: &Value) {
        let wanted = v.as_display();
        if let Some(i) = self.options.iter().position(|o| *o == wanted) {
            self.index = i;
        }
    }

    fn draw(&self, f: &mut Frame, rect: Rect, focused: bool) {
        let has_error = self.error.is_some();
        let body_height = rect.height.saturating_sub(u16::from(has_error));
        let body = Rect {
            height: body_height,
            ..rect
        };

        let block = Block::bordered()
            .border_style(if has_error {
                self.theme.error()
            } else {
                self.theme.block(focused)
            })
            .title_style(self.theme.title(focused))
            .title(format!(" {} ", self.label));

        let inner = block.inner(body);
        f.render_widget(block, body);
        f.render_widget(
            Paragraph::new(format!("‹ {} ›", self.current())).style(self.theme.text()),
            inner,
        );

        if let Some(msg) = &self.error {
            let err_rect = Rect {
                y: body.y + body.height,
                height: 1,
                ..rect
            };
            f.render_widget(
                Paragraph::new(msg.as_str()).style(self.theme.error()),
                err_rect,
            );
        }
    }

    fn event(&mut self, ev: &Event, keys: &KeyConfig) -> Result<EventState> {
        let Event::Key(k) = ev else {
            return Ok(EventState::NotConsumed);
        };
        if self.options.is_empty() {
            return Ok(EventState::NotConsumed);
        }
        let b = &keys.keys;

        if key_match(k, b.move_left) {
            self.index = (self.index + self.options.len() - 1) % self.options.len();
        } else if key_match(k, b.move_right) || key_match(k, b.confirm) {
            self.index = (self.index + 1) % self.options.len();
        } else {
            return Ok(EventState::NotConsumed);
        }
        Ok(EventState::Consumed)
    }

    fn set_error(&mut self, msg: Option<String>) {
        self.error = msg;
    }

    fn desired_height(&self) -> u16 {
        3 + u16::from(self.error.is_some())
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
