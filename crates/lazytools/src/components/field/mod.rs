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

/// A field in the form. `ToolFormComponent` only talks through this trait, so adding
/// a new `FieldKind` means adding a widget — without touching the form.
pub trait FieldWidget {
    fn key(&self) -> &'static str;
    fn value(&self) -> Value;
    fn set_value(&mut self, v: &Value);
    fn draw(&self, f: &mut Frame, rect: Rect, focused: bool);
    fn event(&mut self, ev: &Event, keys: &KeyConfig) -> Result<EventState>;
    fn set_error(&mut self, msg: Option<String>);
    /// Desired height, already accounting for the border and the error line (if any).
    fn desired_height(&self) -> u16;
    /// Output is read-only: it can receive focus (for copying in P3) but can't be edited.
    fn is_readonly(&self) -> bool;
}

/// Builds a widget from a spec `Field`. This is the **only place** that maps
/// `FieldKind` → widget; the `match` has no `_` arm, so adding a new `FieldKind`
/// will make the compiler catch it right here instead of falling through to a silent fallback.
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
