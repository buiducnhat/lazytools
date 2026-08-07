use anyhow::Result;
use lazytools_core::spec::Field;
use lazytools_core::value::Value;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};

use super::FieldWidget;
use crate::components::EventState;
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::ui::SharedTheme;

pub struct ToggleWidget {
    key: &'static str,
    label: &'static str,
    on: bool,
    error: Option<String>,
    theme: SharedTheme,
}

impl ToggleWidget {
    pub fn new(field: &Field, theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            on: false,
            error: None,
            theme,
        }
    }
}

impl FieldWidget for ToggleWidget {
    fn key(&self) -> &'static str {
        self.key
    }

    fn value(&self) -> Value {
        Value::Bool(self.on)
    }

    fn set_value(&mut self, v: &Value) {
        self.on = match v {
            Value::Bool(b) => *b,
            other => matches!(other.as_display().as_str(), "true" | "1" | "yes"),
        };
    }

    fn draw(&self, f: &mut Frame, rect: Rect, focused: bool) {
        let has_error = self.error.is_some();
        let body = Rect {
            height: rect.height.saturating_sub(u16::from(has_error)),
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
            Paragraph::new(if self.on { "[x]" } else { "[ ]" }).style(self.theme.text()),
            inner,
        );

        if let Some(msg) = &self.error {
            f.render_widget(
                Paragraph::new(msg.as_str()).style(self.theme.error()),
                Rect {
                    y: body.y + body.height,
                    height: 1,
                    ..rect
                },
            );
        }
    }

    fn event(&mut self, ev: &Event, keys: &KeyConfig) -> Result<EventState> {
        let Event::Key(k) = ev else {
            return Ok(EventState::NotConsumed);
        };
        let toggles = key_match(k, keys.keys.confirm) || typed_char(k).is_some_and(|c| c == ' ');
        if !toggles {
            return Ok(EventState::NotConsumed);
        }
        self.on = !self.on;
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

    fn event_mouse(
        &mut self,
        _col: u16,
        _row: u16,
        _inner: Rect,
        _keys: &KeyConfig,
    ) -> Result<EventState> {
        self.on = !self.on;
        Ok(EventState::Consumed)
    }
}
