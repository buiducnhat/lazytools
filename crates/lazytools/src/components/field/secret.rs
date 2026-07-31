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

/// Field bí mật (khóa HMAC…): **luôn** render dạng che.
///
/// Giá trị thật chỉ đi ra qua `value()` để nạp vào `Inputs`. Không log, không
/// đưa vào snapshot, không ghi ra đâu cả. v0.1 không có persistence nên rủi ro
/// rò rỉ đã nhỏ sẵn — nguyên tắc này giữ cho nó vẫn nhỏ khi thêm tính năng sau.
pub struct SecretWidget {
    key: &'static str,
    label: &'static str,
    area: TextArea,
    error: Option<String>,
    theme: SharedTheme,
}

impl SecretWidget {
    pub fn new(field: &Field, theme: SharedTheme) -> Self {
        Self {
            key: field.key,
            label: field.label,
            area: TextArea::new(true),
            error: None,
            theme,
        }
    }
}

impl FieldWidget for SecretWidget {
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

        // Chỉ độ dài bị lộ, không bao giờ lộ nội dung.
        let masked = "•".repeat(self.area.value().chars().count());
        f.render_widget(Paragraph::new(masked).style(self.theme.text()), inner);

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
        3 + u16::from(self.error.is_some())
    }

    fn is_readonly(&self) -> bool {
        false
    }
}
