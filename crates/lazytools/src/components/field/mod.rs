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

    /// Whether this widget needs the confirm key for itself, so the form must not
    /// swallow it as "run the tool". Only a multiline text field does — it needs
    /// `Enter` for line breaks. Defaulted to `false` because that is correct for
    /// every other widget: none of them does anything with confirm.
    fn wants_confirm_key(&self) -> bool {
        false
    }

    /// Handles a mouse click at `(col, row)` within this widget's **inner** area.
    /// Returns `Consumed` when the click changed the widget's value (e.g., a toggle).
    /// Default: `NotConsumed` — focus-only widgets don't need to override this.
    fn event_mouse(
        &mut self,
        _col: u16,
        _row: u16,
        _inner: Rect,
        _keys: &KeyConfig,
    ) -> Result<EventState> {
        Ok(EventState::NotConsumed)
    }
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
