//! Two core traits, shaped after gitui. Anything that can be drawn is a
//! `DrawableComponent`; anything that receives input events and declares its own commands is a `Component`.

pub mod cmdbar;
pub mod field;
pub mod palette;
pub mod sidebar;
pub mod tool_form;

use std::cell::Cell;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::layout::Rect;

/// A component's last drawn `Rect`, written at the start of `draw`. The App reads this
/// during event dispatch to decide whether a click landed inside the component.
///
/// `Cell` rather than `RefCell` because `draw(&self, ...)` only has a shared reference —
/// writing the rect inside `draw` needs interior mutability. Resetting to `Rect::default()`
/// at the top of every `draw` is what keeps a popup that was visible last frame but is
/// hidden this frame from claiming a click against a stale rect.
pub type LastArea = Cell<Rect>;

pub trait DrawableComponent {
    fn draw(&self, f: &mut Frame, rect: Rect) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventState {
    Consumed,
    NotConsumed,
}

impl EventState {
    pub fn is_consumed(self) -> bool {
        self == Self::Consumed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBlocking {
    /// The component is currently claiming keys — don't ask the following components anymore.
    Blocking,
    PassingOn,
}

/// A command a component advertises. Both the cmdbar and the help popup are built
/// from this, so the displayed hint never drifts from the actual key-handling code.
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Pre-rendered key string, e.g. `^P` — always taken from `KeyConfig`.
    pub key: String,
    pub label: &'static str,
    /// Group shown in the help popup — the help popup was built in Phase 03.
    pub group: &'static str,
    pub enabled: bool,
    pub order: i8,
}

impl CommandInfo {
    pub fn new(key: String, label: &'static str, group: &'static str) -> Self {
        Self {
            key,
            label,
            group,
            enabled: true,
            order: 0,
        }
    }

    pub fn order(mut self, order: i8) -> Self {
        self.order = order;
        self
    }
}

pub trait Component {
    fn commands(&self, out: &mut Vec<CommandInfo>, force_all: bool) -> CommandBlocking;
    fn event(&mut self, ev: &Event) -> Result<EventState>;
    fn focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);

    fn is_visible(&self) -> bool {
        true
    }
    fn hide(&mut self) {}
    /// Used by popups that open on demand (palette/help in P3, the file picker in P5).
    fn show(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Passes an event through the components in turn, stopping at the first one that consumes it.
pub fn event_pump(ev: &Event, components: &mut [&mut dyn Component]) -> Result<EventState> {
    for c in components.iter_mut() {
        if c.event(ev)?.is_consumed() {
            return Ok(EventState::Consumed);
        }
    }
    Ok(EventState::NotConsumed)
}

/// Collects `CommandInfo` from the currently visible components, stopping at a `Blocking` component.
pub fn command_pump(out: &mut Vec<CommandInfo>, force_all: bool, components: &[&dyn Component]) {
    for c in components {
        if !c.is_visible() && !force_all {
            continue;
        }
        if c.commands(out, force_all) == CommandBlocking::Blocking {
            break;
        }
    }
}
