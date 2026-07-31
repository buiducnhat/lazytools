use anyhow::Result;
use lazytools_core::spec::Field;
use lazytools_core::value::Value;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use super::FieldWidget;
use super::textarea::TextArea;
use crate::components::EventState;
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::ui::SharedTheme;

const MULTILINE_HEIGHT: u16 = 8;
const SINGLE_LINE_HEIGHT: u16 = 3;

pub struct TextWidget {
    key: &'static str,
    label: &'static str,
    area: TextArea,
    multiline: bool,
    readonly: bool,
    error: Option<String>,
    theme: SharedTheme,
}

impl TextWidget {
    pub fn new(field: &Field, multiline: bool, readonly: bool, theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            area: TextArea::new(!multiline),
            multiline,
            readonly,
            error: None,
            theme,
        }
    }

    pub fn len_bytes(&self) -> usize {
        self.area.len_bytes()
    }
}

impl FieldWidget for TextWidget {
    fn key(&self) -> &'static str {
        self.key
    }

    fn value(&self) -> Value {
        Value::Text(self.area.value())
    }

    fn set_value(&mut self, v: &Value) {
        self.area.set_value(&v.as_display());
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

        let (rows, cursor) = self.area.render(inner.width, inner.height);
        let lines: Vec<Line> = rows.into_iter().map(Line::from).collect();
        f.render_widget(Paragraph::new(lines).style(self.theme.text()), inner);

        if focused && !self.readonly {
            f.set_cursor_position((inner.x + cursor.0, inner.y + cursor.1));
        }

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
        if self.readonly {
            return Ok(EventState::NotConsumed);
        }

        // Bracketed paste: the whole block lands in one go, not mistaken for individual keypresses.
        if let Event::Paste(text) = ev {
            self.area.insert_str(text);
            return Ok(EventState::Consumed);
        }

        let Event::Key(k) = ev else {
            return Ok(EventState::NotConsumed);
        };
        let b = &keys.keys;

        if key_match(k, b.backspace) {
            self.area.backspace();
        } else if key_match(k, b.delete) {
            self.area.delete();
        } else if key_match(k, b.move_left) {
            self.area.move_left();
        } else if key_match(k, b.move_right) {
            self.area.move_right();
        } else if key_match(k, b.line_start) || key_match(k, b.line_start_alt) {
            self.area.move_line_start();
        } else if key_match(k, b.line_end) {
            self.area.move_line_end();
        } else if key_match(k, b.delete_to_start) {
            self.area.delete_to_line_start();
        } else if self.multiline && key_match(k, b.move_up_alt) {
            self.area.move_up();
        } else if self.multiline && key_match(k, b.move_down_alt) {
            self.area.move_down();
        } else if self.multiline && key_match(k, b.confirm) {
            self.area.insert_newline();
        } else if let Some(c) = typed_char(k) {
            self.area.insert_char(c);
        } else {
            return Ok(EventState::NotConsumed);
        }
        Ok(EventState::Consumed)
    }

    fn set_error(&mut self, msg: Option<String>) {
        self.error = msg;
    }

    fn desired_height(&self) -> u16 {
        let base = if self.multiline {
            MULTILINE_HEIGHT
        } else {
            SINGLE_LINE_HEIGHT
        };
        base + u16::from(self.error.is_some())
    }

    fn is_readonly(&self) -> bool {
        self.readonly
    }
}
