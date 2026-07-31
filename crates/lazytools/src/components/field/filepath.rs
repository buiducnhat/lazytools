use std::path::Path;

use anyhow::Result;
use lazytools_core::spec::Field;
use lazytools_core::value::Value;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};

use super::FieldWidget;
use super::textarea::TextArea;
use crate::components::EventState;
use crate::keys::{KeyConfig, key_match, typed_char};
use crate::ui::SharedTheme;

/// Ô nhập đường dẫn. P3 chỉ nhập tay + cảnh báo khi `must_exist` mà file không
/// có; nút mở file picker được nối ở Phase 05.
pub struct FilePathWidget {
    key: &'static str,
    label: &'static str,
    area: TextArea,
    must_exist: bool,
    error: Option<String>,
    theme: SharedTheme,
}

impl FilePathWidget {
    pub fn new(field: &Field, must_exist: bool, theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            area: TextArea::new(true),
            must_exist,
            error: None,
            theme,
        }
    }

    /// Cảnh báo tại chỗ, tách khỏi `error` do tool trả về.
    fn missing_file(&self) -> bool {
        let value = self.area.value();
        self.must_exist && !value.is_empty() && !Path::new(&value).exists()
    }
}

impl FieldWidget for FilePathWidget {
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
        let note = self.error.clone().or_else(|| {
            self.missing_file()
                .then(|| "file không tồn tại".to_string())
        });
        let has_note = note.is_some();
        let body = Rect {
            height: rect.height.saturating_sub(u16::from(has_note)),
            ..rect
        };

        let block = Block::bordered()
            .border_style(if has_note {
                self.theme.error()
            } else {
                self.theme.block(focused)
            })
            .title_style(self.theme.title(focused))
            .title(format!(" {} ", self.label));

        let inner = block.inner(body);
        f.render_widget(block, body);

        let (rows, cursor) = self.area.render(inner.width, inner.height);
        f.render_widget(
            Paragraph::new(rows.join("")).style(self.theme.text()),
            inner,
        );
        if focused {
            f.set_cursor_position((inner.x + cursor.0, inner.y + cursor.1));
        }

        if let Some(msg) = note {
            f.render_widget(
                Paragraph::new(msg).style(self.theme.error()),
                Rect {
                    y: body.y + body.height,
                    height: 1,
                    ..rect
                },
            );
        }
    }

    fn event(&mut self, ev: &Event, keys: &KeyConfig) -> Result<EventState> {
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
        3 + u16::from(self.error.is_some() || self.missing_file())
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
