//! Kênh để component con nói ngược lên `App` mà không tạo tham chiếu vòng.
//! Biến thể của `InternalEvent` được thêm dần theo phase cần tới.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use bitflags::bitflags;
use lazytools_core::ToolError;

#[derive(Debug)]
pub enum InternalEvent {
    SelectTool(&'static str),
    /// Một input đổi giá trị → hẹn giờ chạy lại tool (debounce).
    InputChanged,
    /// `RunMode::OnDemand` được kích bằng phím.
    RunRequested,
    OpenPalette,
    ClosePalette,
    ShowHelp,
    CopyToClipboard(String),
    ShowMsg(String),
    ShowError(ToolError),
    Quit,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NeedsUpdate: u32 {
        const ALL      = 0b0001;
        /// Chạy lại tool.
        const OUTPUT   = 0b0010;
        /// Dựng lại hint của cmdbar.
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
