pub mod filepath;
pub mod number;
pub mod secret;
pub mod select;
pub mod text;
pub mod textarea;
pub mod toggle;

use anyhow::Result;
use lazytools_core::spec::{Field, FieldKind};
use lazytools_core::value::Value;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

use super::EventState;
use crate::keys::KeyConfig;
use crate::ui::SharedTheme;

/// Một ô trong form. `ToolFormComponent` chỉ nói chuyện qua trait này, nên thêm
/// một `FieldKind` mới là thêm một widget — không đụng tới form.
pub trait FieldWidget {
    fn key(&self) -> &'static str;
    fn value(&self) -> Value;
    fn set_value(&mut self, v: &Value);
    fn draw(&self, f: &mut Frame, rect: Rect, focused: bool);
    fn event(&mut self, ev: &Event, keys: &KeyConfig) -> Result<EventState>;
    fn set_error(&mut self, msg: Option<String>);
    /// Chiều cao mong muốn, đã tính cả viền và dòng lỗi (nếu có).
    fn desired_height(&self) -> u16;
    /// Output là chỉ-đọc: nhận focus được (để copy ở P3) nhưng không sửa được.
    fn is_readonly(&self) -> bool;
}

/// Dựng widget từ một `Field` của spec. Đây là **chỗ duy nhất** ánh xạ
/// `FieldKind` → widget; `match` không có nhánh `_` nên thêm một `FieldKind`
/// mới sẽ làm compiler bắt lỗi ngay tại đây thay vì rơi vào fallback im lặng.
pub fn build(field: &Field, theme: SharedTheme, readonly: bool) -> Box<dyn FieldWidget> {
    match &field.kind {
        FieldKind::Text { multiline, .. } => {
            Box::new(text::TextWidget::new(field, *multiline, readonly, theme))
        }
        FieldKind::Select { options } => Box::new(select::SelectWidget::new(field, options, theme)),
        FieldKind::Secret => Box::new(secret::SecretWidget::new(field, theme)),
        FieldKind::Number { min, max } => {
            Box::new(number::NumberWidget::new(field, *min, *max, theme))
        }
        FieldKind::Toggle => Box::new(toggle::ToggleWidget::new(field, theme)),
        FieldKind::FilePath { must_exist } => {
            Box::new(filepath::FilePathWidget::new(field, *must_exist, theme))
        }
    }
}
