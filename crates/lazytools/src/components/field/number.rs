use anyhow::Result;
use lazytools_core::spec::Field;
use lazytools_core::value::Value;
use ratatui::crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};

use super::FieldWidget;
use crate::components::EventState;
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::ui::SharedTheme;

/// Bounded integer. The value is always clamped into `[min, max]` right at input time,
/// so the tool never receives a number outside the range declared in the spec.
pub struct NumberWidget {
    key: &'static str,
    label: &'static str,
    value: i64,
    min: i64,
    max: i64,
    error: Option<String>,
    theme: SharedTheme,
}

impl NumberWidget {
    pub fn new(field: &Field, min: i64, max: i64, theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            value: min,
            min,
            max,
            error: None,
            theme,
        }
    }

    fn set_clamped(&mut self, v: i64) {
        self.value = v.clamp(self.min, self.max);
    }
}

impl FieldWidget for NumberWidget {
    fn key(&self) -> &'static str {
        self.key
    }

    fn value(&self) -> Value {
        Value::Num(self.value)
    }

    fn set_value(&mut self, v: &Value) {
        let n = match v {
            Value::Num(n) => *n,
            other => other.as_display().parse().unwrap_or(self.min),
        };
        self.set_clamped(n);
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
            Paragraph::new(format!("‹ {} ›  ({}–{})", self.value, self.min, self.max))
                .style(self.theme.text()),
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
        let b = &keys.keys;

        if key_match(k, b.move_left) {
            self.set_clamped(self.value.saturating_sub(1));
        } else if key_match(k, b.move_right) {
            self.set_clamped(self.value.saturating_add(1));
        } else if key_match(k, b.backspace) {
            self.set_clamped(self.value / 10);
        } else if let Some(c) = typed_char(k).filter(char::is_ascii_digit) {
            let digit = i64::from(c as u8 - b'0');
            // Input outside the range is blocked right there instead of erroring later.
            let candidate = self.value.saturating_mul(10).saturating_add(digit);
            self.set_clamped(candidate);
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

    fn event_mouse(
        &mut self,
        col: u16,
        _row: u16,
        inner: Rect,
        _keys: &KeyConfig,
    ) -> Result<EventState> {
        // ‹ is the first character, › is the last.
        if col <= inner.x + 1 {
            self.set_clamped(self.value.saturating_sub(1));
        } else if col >= inner.x + inner.width.saturating_sub(1) {
            self.set_clamped(self.value.saturating_add(1));
        } else {
            // Click on the value body: same as move_right.
            self.set_clamped(self.value.saturating_add(1));
        }
        Ok(EventState::Consumed)
    }
}
