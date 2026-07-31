//! A channel for a child component to talk back up to `App` without creating a
//! reference cycle. `InternalEvent` variants get added incrementally as each
//! phase needs them.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use bitflags::bitflags;
use lazytools_core::ToolError;

#[derive(Debug)]
pub enum InternalEvent {
    SelectTool(&'static str),
    /// An input's value changed → schedule a debounced tool re-run.
    InputChanged,
    /// `RunMode::OnDemand` was triggered via a key press.
    RunRequested,
    OpenPalette,
    ClosePalette,
    ShowHelp,
    CopyToClipboard(String),
    /// The user picked a file in the picker.
    OpenFile(std::path::PathBuf),
    /// Opens the save popup for the currently focused output value.
    SaveOutput(String),
    ShowMsg(String),
    ShowError(ToolError),
    Quit,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NeedsUpdate: u32 {
        const ALL      = 0b0001;
        /// Re-run the tool.
        const OUTPUT   = 0b0010;
        /// Rebuild the cmdbar hints.
        const COMMANDS = 0b0100;
        const SIDEBAR  = 0b1000;
    }
}

#[derive(Clone, Default)]
pub struct Queue(Rc<RefCell<VecDeque<InternalEvent>>>);

impl Queue {
    pub fn push(&self, ev: InternalEvent) {
        self.0.borrow_mut().push_back(ev);
    }

    pub fn pop(&self) -> Option<InternalEvent> {
        self.0.borrow_mut().pop_front()
    }
}
